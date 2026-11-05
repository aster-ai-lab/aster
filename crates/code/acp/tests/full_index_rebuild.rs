//! Opt-in full-repository retrieval rebuild with isolated durable state.

use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};

use aster_core::{
    Agent, SessionOptions, WorkspaceRetrievalOptions, WorkspaceRetrievalPhase,
    WorkspaceSemanticSearchRequest,
};

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[ignore = "full repository index rebuild; resource intensive"]
async fn full_repository_index_rebuild_is_complete_and_searchable() {
    let _ = tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .with_ansi(false)
        .try_init();
    let workspace = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../..")
        .canonicalize()
        .expect("repository root");
    let state = tempfile::tempdir().expect("isolated index state");
    aster_core::state_root::set_state_root_override(state.path()).expect("isolated state root");
    let provider =
        Arc::new(aster_embedding::LightTierProvider::load().expect("embedded light-tier provider"));
    let config = state.path().join("config.acl");
    std::fs::write(
        &config,
        "providers \"mock\" { base_url = \"http://127.0.0.1:1/v1\" api_key = \"test\" models \"gpt\" {} }\ndefault_model = \"mock/gpt\"\n",
    )
    .expect("mock provider config");
    let launch =
        aster_acp::load_launch_config(workspace.clone(), Some(config)).expect("launch config");
    let agent = Agent::from_config(launch.config).await.expect("core agent");
    let options = SessionOptions::new()
        .with_session_id("full-index-rebuild")
        .with_model("mock/gpt")
        .with_workspace_retrieval(WorkspaceRetrievalOptions::new(provider));
    let session = agent
        .session_async(workspace.to_string_lossy().to_string(), Some(options))
        .await
        .expect("retrieval session");

    let started = Instant::now();
    let mut last_status = String::new();
    let status = tokio::time::timeout(Duration::from_secs(240), async {
        loop {
            let status = session.workspace_retrieval_status();
            let rendered = serde_json::to_string(&status).expect("status json");
            if rendered != last_status {
                eprintln!("REBUILD_PROGRESS {rendered}");
                last_status = rendered.clone();
            }
            match status.phase {
                WorkspaceRetrievalPhase::Ready => break status,
                WorkspaceRetrievalPhase::Degraded if status.queue_depth == 0 => break status,
                _ => tokio::time::sleep(Duration::from_millis(250)).await,
            }
        }
    })
    .await
    .expect("index rebuild exceeded 240 seconds");
    assert!(status.eligible_files > 0, "{status:?}");
    eprintln!(
        "REBUILD_STOP {} {}",
        serde_json::to_string(&status).expect("status json"),
        started.elapsed().as_secs_f64()
    );

    match session
        .semantic_search(WorkspaceSemanticSearchRequest::new("GREP_SCAN_TIMEOUT").with_limit(3))
        .await
    {
        Ok(result) => {
            assert!(result
                .hits
                .iter()
                .all(|hit| !hit.chunk.path.starts_with("docs/evidence")));
            eprintln!(
                "REBUILD_HITS {} {:?}",
                result.hits.len(),
                result
                    .hits
                    .iter()
                    .map(|hit| hit.chunk.path.as_ref())
                    .collect::<Vec<_>>()
            );
        }
        Err(error) => eprintln!("REBUILD_SEARCH_ERROR {error}"),
    }

    session.close().await;
    agent.close().await;
    assert_eq!(status.phase, WorkspaceRetrievalPhase::Ready, "{status:?}");
    assert_eq!(status.failed_files, 0, "{status:?}");
    assert_eq!(status.indexed_files, status.eligible_files, "{status:?}");
    assert!(status.vector_records > 0, "{status:?}");
}
