//! Reproducible Aster chunk/rerank and rendered-evidence qualification.
//! The orthogonal-vector control isolates lexical/ranking behavior; it makes no
//! claim about real embedding quality or tokenizer-measured token savings.

use aster_core::{
    Agent, ChunkCatalogLimits, ChunkingConfig, CodeConfig, EmbeddingBatchRequest,
    EmbeddingBatchResponse, EmbeddingProvider, EmbeddingProviderDescriptor, EmbeddingProviderError,
    EmbeddingVector, RecursiveChunkingOptions, SessionOptions, WorkspaceChunkCatalog,
    WorkspaceChunkingStrategy, WorkspaceDirEntry, WorkspaceFileSystem,
    WorkspaceHybridSearchRequest, WorkspaceLexicalEngine, WorkspacePath, WorkspaceRerankOptions,
    WorkspaceResult, WorkspaceRetrievalOptions, WorkspaceRetrievalPhase, WorkspaceServices,
    WorkspaceWriteOutcome,
};
use anyhow::{bail, Context, Result};
use async_trait::async_trait;
use serde::Deserialize;
use serde_json::json;
use std::collections::BTreeSet;
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio_util::sync::CancellationToken;

const CORPUS: &str = include_str!("../tests/fixtures/workspace-retrieval-aster-v1/corpus.json");

#[derive(Deserialize)]
struct Corpus {
    schema_version: u32,
    documents: Vec<Document>,
    queries: Vec<Query>,
}

#[derive(Deserialize)]
struct Document {
    path: String,
    prefix_lines: usize,
    content: String,
}

#[derive(Deserialize)]
struct Query {
    id: String,
    category: String,
    query: String,
    relevant_path: Option<String>,
    evidence: Option<String>,
    #[serde(default)]
    require_first: bool,
}

#[derive(Default)]
struct OrthogonalProvider {
    requests: AtomicUsize,
    inputs: AtomicUsize,
    querying: AtomicBool,
}

struct MeasuredFiles {
    inner: Arc<dyn WorkspaceFileSystem>,
    reads: AtomicUsize,
    read_nanos: AtomicU64,
}

#[async_trait]
impl WorkspaceFileSystem for MeasuredFiles {
    async fn read_text(&self, path: &WorkspacePath) -> WorkspaceResult<String> {
        let started = Instant::now();
        let result = self.inner.read_text(path).await;
        self.reads.fetch_add(1, Ordering::Relaxed);
        self.read_nanos.fetch_add(
            started.elapsed().as_nanos().min(u64::MAX as u128) as u64,
            Ordering::Relaxed,
        );
        result
    }

    async fn write_text(
        &self,
        path: &WorkspacePath,
        content: &str,
    ) -> WorkspaceResult<WorkspaceWriteOutcome> {
        self.inner.write_text(path, content).await
    }

    async fn list_dir(&self, path: &WorkspacePath) -> WorkspaceResult<Vec<WorkspaceDirEntry>> {
        self.inner.list_dir(path).await
    }
}

#[async_trait]
impl EmbeddingProvider for OrthogonalProvider {
    fn descriptor(&self) -> EmbeddingProviderDescriptor {
        EmbeddingProviderDescriptor::new("fixture", "orthogonal-vector-control", 8)
    }

    async fn embed(
        &self,
        request: EmbeddingBatchRequest,
        cancellation: CancellationToken,
    ) -> std::result::Result<EmbeddingBatchResponse, EmbeddingProviderError> {
        if cancellation.is_cancelled() {
            return Err(EmbeddingProviderError::Cancelled);
        }
        self.requests.fetch_add(1, Ordering::Relaxed);
        self.inputs
            .fetch_add(request.inputs().len(), Ordering::Relaxed);
        Ok(EmbeddingBatchResponse::new(
            self.descriptor(),
            request
                .inputs()
                .iter()
                .map(|input| {
                    let mut vector = vec![0.0; 8];
                    vector[usize::from(self.querying.load(Ordering::Acquire))] = 1.0;
                    EmbeddingVector::new(input.id(), vector)
                })
                .collect(),
        ))
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    let corpus: Corpus = serde_json::from_str(CORPUS)?;
    if corpus.schema_version != 1 || corpus.queries.len() < 9 {
        bail!("invalid locked Aster corpus");
    }
    let mut profiles = Vec::new();
    for recursive in [false, true] {
        for rerank in [false, true] {
            profiles.push(profile(&corpus, recursive, rerank).await?);
        }
    }
    println!(
        "{}",
        serde_json::to_string_pretty(&json!({
            "schemaVersion": 1, "profile": "aster-retrieval-v1",
            "fixtureSha256": sha256::digest(CORPUS),
            "build": if cfg!(debug_assertions) { "debug" } else { "release" },
            "machine": {"os": std::env::consts::OS, "arch": std::env::consts::ARCH},
            "provider": "orthogonal document/query lexical/ranking control; not real semantic quality",
            "tokenMeasurement": "unavailable; bytes reported without converting to tokens",
            "profiles": profiles,
        }))?
    );
    if profiles.iter().any(|profile| profile["qualified"] == false) {
        bail!("Aster strategy matrix failed; see retained per-profile failures");
    }
    Ok(())
}

async fn profile(corpus: &Corpus, recursive: bool, rerank: bool) -> Result<serde_json::Value> {
    let root = tempfile::tempdir()?;
    let strategy = if recursive {
        WorkspaceChunkingStrategy::Recursive(RecursiveChunkingOptions::new(512, 64)?)
    } else {
        WorkspaceChunkingStrategy::Lines
    };
    let catalog = WorkspaceChunkCatalog::new_with_strategy_and_engine(
        strategy,
        ChunkingConfig::default(),
        ChunkCatalogLimits::default(),
        WorkspaceLexicalEngine::Portable,
    )?;
    let started = Instant::now();
    let mut source_bytes = 0;
    for (revision, document) in corpus.documents.iter().enumerate() {
        let path = root.path().join(&document.path);
        std::fs::create_dir_all(path.parent().context("fixture parent")?)?;
        let content = format!(
            "{}{}",
            "// unrelated source\n".repeat(document.prefix_lines),
            document.content
        );
        source_bytes += content.len();
        std::fs::write(path, &content)?;
        catalog.replace_file(
            &WorkspacePath::from_normalized(&document.path),
            None,
            revision as u64 + 1,
            &content,
        )?;
    }
    let admission_ms = started.elapsed().as_secs_f64() * 1000.0;
    let filesystem = Arc::new(MeasuredFiles {
        inner: WorkspaceServices::local(root.path()).fs(),
        reads: AtomicUsize::new(0),
        read_nanos: AtomicU64::new(0),
    });
    let services = WorkspaceServices::builder(
        aster_core::WorkspaceRef::new("fixture", "fixture://aster"),
        filesystem.clone(),
    )
    .chunk_catalog(catalog.clone())
    .build();
    let provider = Arc::new(OrthogonalProvider::default());
    let options = WorkspaceRetrievalOptions::new(provider.clone()).with_rerank_options(if rerank {
        WorkspaceRerankOptions::deterministic()
    } else {
        WorkspaceRerankOptions::default()
    });
    let config = CodeConfig::from_acl(
        r#"
        default_model = "fixture/model"
        providers "fixture" {
            api_key = "fixture-only"
            base_url = "http://127.0.0.1:9/v1"
            models "model" {}
        }
    "#,
    )?;
    let agent = Agent::from_config(config).await?;
    let started = Instant::now();
    let session = agent
        .session_async(
            root.path().to_string_lossy(),
            Some(
                SessionOptions::new()
                    .with_workspace_backend(services)
                    .with_workspace_retrieval(options),
            ),
        )
        .await?;
    let construction_ms = started.elapsed().as_secs_f64() * 1000.0;
    let started = Instant::now();
    let ready = tokio::time::timeout(Duration::from_secs(10), async {
        loop {
            let status = session.workspace_retrieval_status();
            match status.phase {
                WorkspaceRetrievalPhase::Ready => return Ok(status),
                WorkspaceRetrievalPhase::Degraded => {
                    bail!("fixture projection degraded: {status:?}")
                }
                _ => tokio::time::sleep(Duration::from_millis(5)).await,
            }
        }
    })
    .await
    .context("bounded fixture readiness")??;
    let ready_ms = started.elapsed().as_secs_f64() * 1000.0;
    let document_requests = provider.requests.load(Ordering::Relaxed);
    if provider.inputs.load(Ordering::Relaxed) != catalog.snapshot()?.chunk_count()
        || document_requests != ready.batching.document_provider_requests
        || document_requests != ready.batching.batch_limit_lower_bound
    {
        bail!("document request accounting or batch amplification regressed");
    }
    // Valid unit vectors with zero similarity keep semantic evidence neutral.
    provider.querying.store(true, Ordering::Release);
    let mut queries = Vec::new();
    let mut failures = Vec::new();
    let mut latencies = Vec::new();
    for query in &corpus.queries {
        let request = WorkspaceHybridSearchRequest::new(&query.query);
        let reads_before = filesystem.reads.load(Ordering::Relaxed);
        let nanos_before = filesystem.read_nanos.load(Ordering::Relaxed);
        let structured_started = Instant::now();
        let result = session.hybrid_search(request.clone()).await?;
        let structured_ms = structured_started.elapsed().as_secs_f64() * 1000.0;
        let source_read_ms =
            (filesystem.read_nanos.load(Ordering::Relaxed) - nanos_before) as f64 / 1_000_000.0;
        let source_reads = filesystem.reads.load(Ordering::Relaxed) - reads_before;
        let paths = result
            .hits
            .iter()
            .map(|hit| hit.chunk.path.as_ref())
            .collect::<Vec<_>>();
        let rank = query
            .relevant_path
            .as_deref()
            .and_then(|path| paths.iter().position(|hit| *hit == path));
        if query.relevant_path.is_some() && rank.is_none() {
            failures.push(format!("{} lost relevant source", query.id));
        }
        if query.require_first && rank != Some(0) {
            failures.push(format!("{} displaced its exact identifier", query.id));
        }
        if query.relevant_path.is_none() && !paths.is_empty() {
            failures.push(format!(
                "{} manufactured negative-control evidence",
                query.id
            ));
        }
        let rendered_started = Instant::now();
        let output = session
            .tool("search", json!({"mode":"hybrid", "query":query.query}))
            .await?;
        let rendered_ms = rendered_started.elapsed().as_secs_f64() * 1000.0;
        if output.exit_code != 0
            || query
                .evidence
                .as_ref()
                .is_some_and(|text| !output.output.contains(text))
        {
            failures.push(format!("{} lost rendered evidence", query.id));
        }
        let legacy_prefix_contains_evidence = query.evidence.as_ref().is_some_and(|text| {
            result.hits.iter().any(|hit| {
                hit.chunk
                    .text
                    .lines()
                    .take(16)
                    .collect::<Vec<_>>()
                    .join("\n")
                    .contains(text)
            })
        });
        for _ in 0..10 {
            let started = Instant::now();
            session.hybrid_search(request.clone()).await?;
            latencies.push(started.elapsed().as_secs_f64() * 1000.0);
        }
        let unique = paths.iter().copied().collect::<BTreeSet<_>>().len();
        queries.push(json!({
            "id": query.id, "category": query.category, "paths": paths,
            "recallAt5": query.relevant_path.as_ref().map(|_| u8::from(rank.is_some_and(|rank| rank < 5))),
            "recallAt10": query.relevant_path.as_ref().map(|_| u8::from(rank.is_some_and(|rank| rank < 10))),
            "reciprocalRank": query.relevant_path.as_ref().map(|_| rank.map_or(0.0, |rank| 1.0 / (rank + 1) as f64)),
            "ndcgAt10": query.relevant_path.as_ref().map(|_| rank.map_or(0.0, |rank| 1.0 / ((rank + 2) as f64).log2())),
            "requireFirst": query.require_first,
            "repeatedFileHits": result.hits.len() - unique,
            "renderedEvidencePresent": query.evidence.as_ref().map(|text| output.output.contains(text)),
            "legacyPrefixEvidencePresent": query.evidence.as_ref().map(|_| legacy_prefix_contains_evidence),
            "responseBytes": output.output.len(), "rerank": result.rerank,
            "structuredQueryMs": structured_ms, "sourceReadMs": source_read_ms,
            "sourceReads": source_reads, "structuredNonReadMs": (structured_ms - source_read_ms).max(0.0),
            "toolRoundTripIncludingRenderingMs": rendered_ms,
        }));
    }
    latencies.sort_by(f64::total_cmp);
    let snapshot = catalog.snapshot()?;
    session.close().await;
    let closed = session.workspace_retrieval_status();
    if closed.vector_records != 0 || closed.vector_bytes != 0 {
        bail!("session close retained vectors");
    }
    Ok(json!({
        "chunking": if recursive { "recursive_512_64" } else { "lines_80" },
        "rerank": if rerank { "deterministic_mmr" } else { "rrf_only" },
        "sourceFiles": corpus.documents.len(), "sourceBytes": source_bytes,
        "chunks": snapshot.chunk_count(), "catalogTextBytes": snapshot.text_bytes(),
        "catalogIndexBytes": snapshot.estimated_index_bytes(), "dimension": 8,
        "admissionMs": admission_ms, "sessionConstructionMs": construction_ms, "semanticReadyMs": ready_ms,
        "documentProviderRequests": document_requests, "documentInputs": ready.batching.document_inputs,
        "batching": ready.batching, "vectorBytes": ready.vector_bytes,
        "queryP50Ms": latencies[latencies.len() / 2], "queryP95Ms": latencies[latencies.len() * 95 / 100],
        "queryTimingIncludes": "control-provider callback, ranking and authoritative source reads; excludes rendering",
        "postCloseVectors": closed.vector_records, "postCloseVectorBytes": closed.vector_bytes,
        "qualified": failures.is_empty(), "failures": failures,
        "queries": queries,
    }))
}
