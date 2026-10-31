#![cfg(test)]

use super::{test_launch, AsterCodeAgent, SessionSetup};
use aster_core::{
    CodeConfig, EmbeddingBatchRequest, EmbeddingBatchResponse, EmbeddingProvider,
    EmbeddingProviderDescriptor, EmbeddingProviderError, ModelConfig, ProviderConfig,
    WorkspaceRetrievalPhase,
};
use async_trait::async_trait;
use std::path::Path;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tokio::sync::Notify;
use tokio_util::sync::CancellationToken;

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn embedded_local_vectors_publish_before_search_and_are_released_on_close() {
    let root = tempfile::tempdir().unwrap();
    std::fs::write(
        root.path().join("settings.rs"),
        "// Read application configuration settings from disk.\npub fn load_settings() {}\n",
    )
    .unwrap();
    std::fs::write(
        root.path().join("weather.md"),
        "The weather is sunny today.\n",
    )
    .unwrap();
    std::fs::write(root.path().join(".env"), "qxz_vector_secret=excluded\n").unwrap();
    let config = fixture_config();
    let agent = aster_core::Agent::from_config(config.clone())
        .await
        .unwrap();
    let provider = Arc::new(aster_embedding::LightTierProvider::load().unwrap());
    let options = host_options(
        root.path(),
        &config,
        "offline-vector-fixture",
        Some(provider),
    );
    let started = std::time::Instant::now();
    let session = open_session(&agent, root.path(), options).await;
    let opened = started.elapsed();
    assert!(session.prepare_workspace_index().await.unwrap());
    let status = tokio::time::timeout(Duration::from_secs(10), async {
        loop {
            let status = session.workspace_retrieval_status();
            if status.phase == WorkspaceRetrievalPhase::Ready && status.indexed_files == 2 {
                break status;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .unwrap_or_else(|_| {
        panic!(
            "vectors must publish without a search triggering the build: {:?}",
            session.workspace_retrieval_status()
        )
    });
    assert!(status.vector_records > 0 && status.vector_bytes > 0);
    assert_eq!(status.coverage_bps, 10_000);
    assert_eq!(status.model.as_ref().unwrap().provider, "light");
    let found = session
        .semantic_search(aster_core::workspace::WorkspaceSemanticSearchRequest::new(
            "parse application configuration",
        ))
        .await
        .unwrap();
    assert!(found
        .hits
        .iter()
        .any(|hit| hit.chunk.path.as_ref() == "settings.rs"));
    assert!(found
        .hits
        .iter()
        .all(|hit| hit.chunk.path.as_ref() != ".env"));
    eprintln!(
        "ORACLE vector-open-ms={} vector-ready-ms={} vector-records={} vector-bytes={}",
        opened.as_millis(),
        started.elapsed().as_millis(),
        status.vector_records,
        status.vector_bytes
    );
    session.close().await;
    let closed = session.workspace_retrieval_status();
    assert_eq!(closed.phase, WorkspaceRetrievalPhase::Closed);
    assert_eq!(closed.vector_records, 0);
    assert!(session
        .semantic_search(aster_core::workspace::WorkspaceSemanticSearchRequest::new(
            "configuration"
        ))
        .await
        .is_err());
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn offline_vectors_index_a_large_fixture_before_search_with_bounded_state() {
    for files in [512, 5_000] {
        let root = tempfile::tempdir().unwrap();
        for index in 0..files - 1 {
            std::fs::write(root.path().join(format!("module_{index:04}.md")),
            format!("# Data module {index}\n\nThis module transforms warehouse inventory records and calculates stock quantities.\n")).unwrap();
        }
        std::fs::write(root.path().join("billing.md"),
        "# Customer billing\n\nLoad payment ledger invoices, reconcile customer charges, and calculate billing balances.\n").unwrap();
        let config = fixture_config();
        let agent = aster_core::Agent::from_config(config.clone())
            .await
            .unwrap();
        let provider = Arc::new(aster_embedding::LightTierProvider::load().unwrap());
        let options = host_options(
            root.path(),
            &config,
            "large-offline-vector-fixture",
            Some(provider),
        );
        let started = std::time::Instant::now();
        let session = open_session(&agent, root.path(), options).await;
        let opened = started.elapsed();
        assert!(session.prepare_workspace_index().await.unwrap());
        let status = tokio::time::timeout(Duration::from_secs(15), async {
            loop {
                let status = session.workspace_retrieval_status();
                if status.phase == WorkspaceRetrievalPhase::Ready && status.indexed_files == files {
                    break status;
                }
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
        })
        .await
        .unwrap_or_else(|_| {
            panic!(
                "large index did not finish: {:?}",
                session.workspace_retrieval_status()
            )
        });
        let ready = started.elapsed();
        assert_eq!(status.coverage_bps, 10_000);
        assert!(status.vector_bytes > 0 && status.vector_bytes < 128 * 1024 * 1024);
        let mut searches = Vec::new();
        for _ in 0..5 {
            let query_started = std::time::Instant::now();
            let found = session
                .semantic_search(aster_core::workspace::WorkspaceSemanticSearchRequest::new(
                    "payment ledger invoices and customer billing",
                ))
                .await
                .unwrap();
            assert!(found
                .hits
                .iter()
                .any(|hit| hit.chunk.path.as_ref() == "billing.md"));
            searches.push(query_started.elapsed().as_micros());
        }
        eprintln!("ORACLE large-vector-files={} open-ms={} ready-ms={} vector-records={} vector-bytes={} search-max-us={}",
        files, opened.as_millis(), ready.as_millis(), status.vector_records, status.vector_bytes, searches.iter().max().unwrap());
        session.close().await;
        assert_eq!(session.workspace_retrieval_status().vector_records, 0);
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn restricted_review_skips_the_host_enabled_embedding_provider() {
    let root = tempfile::tempdir().unwrap();
    let mut launch = test_launch(Some(root.path().join("sessions")));
    launch.config.workspace_retrieval = Some(aster_core::config::WorkspaceRetrievalConfig {
        enabled: true,
        local_cpu: true,
    });
    let model = launch.model_id.clone();
    let agent = AsterCodeAgent::new(launch);
    let options = agent
        .session_options_for(
            "isolated-review",
            &SessionSetup {
                workspace: root.path().to_path_buf(),
                model_id: &model,
                effort: None,
                host: crate::host_context::StudioHost {
                    source_review: true,
                    ..Default::default()
                },
                components: &[],
                mcp_servers: Vec::new(),
            },
            &root.path().join("sessions"),
        )
        .await
        .unwrap();
    assert!(options.workspace_retrieval.is_none());
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn workspace_index_builds_asynchronously_after_acp_session_open_without_a_query() {
    let root = tempfile::tempdir().unwrap();
    let workspace = root.path().join("workspace");
    std::fs::create_dir(&workspace).unwrap();
    std::fs::write(
        workspace.join("source.rs"),
        "pub fn acp_index_marker() {}\n",
    )
    .unwrap();
    let workspace = workspace.canonicalize().unwrap();
    let mut launch = test_launch(Some(root.path().join("sessions")));
    launch.workspace = workspace.clone();
    launch.config.memory_dir = Some(root.path().join("memory"));
    let model = launch.model_id.clone();
    let agent = AsterCodeAgent::new(launch);
    let live = agent
        .open_core_session(
            "automatic-workspace-index",
            SessionSetup {
                workspace: workspace.clone(),
                model_id: &model,
                effort: None,
                host: Default::default(),
                components: &[],
                mcp_servers: Vec::new(),
            },
            false,
        )
        .await
        .unwrap();
    tokio::time::timeout(Duration::from_secs(15), async {
        while !workspace.join(".aster/index/CURRENT").is_file() {
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    })
    .await
    .expect("ACP session must publish its workspace index before the first search");
    let result = live
        .session
        .tool(
            "search",
            serde_json::json!({
                "mode":"bm25","query":"acp_index_marker","limit":1
            }),
        )
        .await
        .unwrap();
    assert_eq!(result.exit_code, 0, "{}", result.output);
    assert!(result.output.contains("acp_index_marker"));
    assert_eq!(
        result.metadata.unwrap()["index_kind"],
        "persistent_aster_vec_fts"
    );
    live.session.close().await;
}

fn fixture_config() -> CodeConfig {
    CodeConfig {
        default_model: Some("fixture/model".to_string()),
        providers: vec![ProviderConfig {
            protocol: None,
            name: "fixture".to_string(),
            api_key: Some("test-key".to_string()),
            base_url: None,
            headers: Default::default(),
            session_id_header: None,
            models: vec![ModelConfig {
                id: "model".to_string(),
                name: "model".to_string(),
                family: "fixture".to_string(),
                api_key: None,
                base_url: None,
                headers: Default::default(),
                session_id_header: None,
                attachment: false,
                reasoning: false,
                tool_call: true,
                temperature: true,
                release_date: None,
                modalities: Default::default(),
                cost: Default::default(),
                limit: Default::default(),
            }],
        }],
        ..Default::default()
    }
}

fn host_options(
    root: &Path,
    config: &CodeConfig,
    session_id: &str,
    embedding: Option<Arc<dyn EmbeddingProvider>>,
) -> aster_core::SessionOptions {
    let setup = SessionSetup {
        workspace: root.to_owned(),
        model_id: "fixture/model",
        effort: None,
        host: Default::default(),
        components: &[],
        mcp_servers: Vec::new(),
    };
    let store = root.join(".aster/store");
    let trust = root.join(".aster/hooks-trust.json");
    let options = AsterCodeAgent::core_session_options(session_id, setup.model_id, setup.effort);
    let mut slots = crate::host_context::merge_studio_host(
        options.prompt_slots.clone().unwrap_or_default(),
        &setup.host,
    );
    AsterCodeAgent::host_session_options_with_hook_roots(
        options,
        &mut slots,
        &setup,
        config,
        &store,
        None,
        Some(&trust),
        embedding,
    )
    .expect("host options")
}

fn capability_note(options: &aster_core::SessionOptions) -> String {
    options
        .prompt_slots
        .as_ref()
        .and_then(|slots| slots.extra.clone())
        .expect("capability status")
}

fn search_surface(session: &aster_core::AgentSession) -> (serde_json::Value, String) {
    let search = session
        .tool_definitions()
        .into_iter()
        .find(|tool| tool.name == "search")
        .expect("search tool");
    (
        search.parameters["properties"]["mode"]["enum"].clone(),
        search.description,
    )
}

struct BlockingEmbed {
    calls: AtomicUsize,
    called: Notify,
    cancellation: Mutex<Option<CancellationToken>>,
}

impl BlockingEmbed {
    fn new() -> Self {
        Self {
            calls: AtomicUsize::new(0),
            called: Notify::new(),
            cancellation: Mutex::new(None),
        }
    }

    async fn wait_for_calls(&self, expected: usize) {
        tokio::time::timeout(Duration::from_secs(5), async {
            while self.calls.load(Ordering::Acquire) < expected {
                self.called.notified().await;
            }
        })
        .await
        .expect("embedding provider was not called");
    }

    fn request_was_cancelled(&self) -> bool {
        self.cancellation
            .lock()
            .expect("cancellation")
            .as_ref()
            .is_some_and(CancellationToken::is_cancelled)
    }
}

#[async_trait]
impl EmbeddingProvider for BlockingEmbed {
    fn descriptor(&self) -> EmbeddingProviderDescriptor {
        EmbeddingProviderDescriptor::new("fixture", "blocked-v1", 2)
    }

    async fn embed(
        &self,
        _request: EmbeddingBatchRequest,
        cancellation: CancellationToken,
    ) -> Result<EmbeddingBatchResponse, EmbeddingProviderError> {
        self.calls.fetch_add(1, Ordering::AcqRel);
        *self.cancellation.lock().expect("cancellation") = Some(cancellation.clone());
        self.called.notify_waiters();
        cancellation.cancelled().await;
        Err(EmbeddingProviderError::Cancelled)
    }
}

struct FailingEmbed;

#[async_trait]
impl EmbeddingProvider for FailingEmbed {
    fn descriptor(&self) -> EmbeddingProviderDescriptor {
        EmbeddingProviderDescriptor::new("fixture", "fail-v1", 2)
    }

    async fn embed(
        &self,
        _request: EmbeddingBatchRequest,
        _cancellation: CancellationToken,
    ) -> Result<EmbeddingBatchResponse, EmbeddingProviderError> {
        Err(EmbeddingProviderError::InvalidRequest)
    }
}

async fn open_session(
    agent: &aster_core::Agent,
    root: &Path,
    options: aster_core::SessionOptions,
) -> aster_core::AgentSession {
    tokio::time::timeout(
        Duration::from_secs(5),
        agent.session_async(root.to_string_lossy().to_string(), Some(options)),
    )
    .await
    .expect("session construction waited for embeddings")
    .expect("session")
}

#[tokio::test]
async fn retrieval_seam_stays_closed_until_a_provider_is_injected() {
    let config = fixture_config();
    let agent = aster_core::Agent::from_config(config.clone())
        .await
        .expect("agent");

    let closed = tempfile::tempdir().expect("workspace");
    let store = closed.path().join("store");
    let setup = SessionSetup {
        workspace: closed.path().to_owned(),
        model_id: "fixture/model",
        effort: None,
        host: Default::default(),
        components: &[],
        mcp_servers: Vec::new(),
    };
    let closed_options =
        AsterCodeAgent::host_session_options("retrieval-seam-off", &setup, &config, &store)
            .expect("host options");
    assert!(closed_options.workspace_retrieval.is_none());
    let closed_note = capability_note(&closed_options);
    assert!(closed_note.contains("语义检索关闭"));
    assert!(!closed_note.contains("semantic, hybrid"));
    let closed_session = open_session(&agent, closed.path(), closed_options).await;
    assert_eq!(
        closed_session.workspace_retrieval_status().phase,
        WorkspaceRetrievalPhase::Disabled
    );
    let (modes, description) = search_surface(&closed_session);
    assert_eq!(modes, serde_json::json!(["grep", "glob", "bm25", "hybrid"]));
    assert!(!description.contains("semantic"));
    assert!(description.contains("hybrid"));
    closed_session.close().await;

    let blocked_root = tempfile::tempdir().expect("workspace");
    std::fs::write(
        blocked_root.path().join("lib.rs"),
        "pub fn semantic_probe() {}\n",
    )
    .expect("source");
    let blocking = Arc::new(BlockingEmbed::new());
    let blocking_port: Arc<dyn EmbeddingProvider> = blocking.clone();
    let blocked_options = host_options(
        blocked_root.path(),
        &config,
        "retrieval-seam-on",
        Some(blocking_port.clone()),
    );
    let attached = blocked_options
        .workspace_retrieval
        .as_ref()
        .expect("retrieval options")
        .embedding_provider();
    assert!(Arc::ptr_eq(&blocking_port, &attached));
    let blocked_note = capability_note(&blocked_options);
    assert!(blocked_note.contains("search modes: grep, glob, bm25, semantic, hybrid."));
    assert!(blocked_note
        .contains("If the index is degraded, semantic leaves the tool definition; hybrid retains available channels."));
    assert!(!blocked_note.contains("语义检索关闭"));
    let blocked_session = open_session(&agent, blocked_root.path(), blocked_options).await;
    assert_eq!(
        blocked_session.workspace_retrieval_status().phase,
        WorkspaceRetrievalPhase::Building
    );
    let (modes, description) = search_surface(&blocked_session);
    assert_eq!(
        modes,
        serde_json::json!(["grep", "glob", "bm25", "semantic", "hybrid"])
    );
    assert!(description.contains("semantic"));
    assert!(description.contains("hybrid"));
    blocking.wait_for_calls(1).await;
    tokio::time::timeout(Duration::from_secs(5), blocked_session.close())
        .await
        .expect("close waited for the blocked embedding");
    assert!(blocking.request_was_cancelled());

    let failed_root = tempfile::tempdir().expect("workspace");
    std::fs::write(
        failed_root.path().join("lib.rs"),
        "pub fn semantic_probe() {}\n",
    )
    .expect("source");
    let failing: Arc<dyn EmbeddingProvider> = Arc::new(FailingEmbed);
    let failed_options = host_options(
        failed_root.path(),
        &config,
        "retrieval-seam-degraded",
        Some(failing),
    );
    let failed_session = open_session(&agent, failed_root.path(), failed_options).await;
    let degraded = tokio::time::timeout(Duration::from_secs(10), async {
        loop {
            let status = failed_session.workspace_retrieval_status();
            if status.phase == WorkspaceRetrievalPhase::Degraded {
                return;
            }
            assert_ne!(status.phase, WorkspaceRetrievalPhase::Closed);
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await;
    if degraded.is_err() {
        panic!(
            "index did not degrade: {:?}",
            failed_session.workspace_retrieval_status()
        );
    }
    let (modes, description) = search_surface(&failed_session);
    assert_eq!(modes, serde_json::json!(["grep", "glob", "bm25", "hybrid"]));
    assert!(description.contains("bm25"));
    assert!(!description.contains("semantic"));
    assert!(description.contains("hybrid"));
    let result = failed_session
        .tool(
            "search",
            serde_json::json!({
                "mode": "hybrid", "query": "semantic_probe", "limit": 1,
            }),
        )
        .await
        .expect("degraded hybrid query");
    assert_eq!(result.exit_code, 0, "{}", result.output);
    assert!(
        result.output.contains("pub fn semantic_probe()"),
        "{}",
        result.output
    );
    assert_eq!(
        result.metadata.as_ref().unwrap()["semantic_status"]["phase"],
        "degraded"
    );
    failed_session.close().await;
}
