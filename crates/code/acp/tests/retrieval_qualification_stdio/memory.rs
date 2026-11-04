//! Actual Core memory-owner qualification, distinct from the ACP workspace route.

use aster_core::{
    DurableMemoryRecallPolicy, DurableMemorySemanticRecall, DurableMemorySemanticRecallPolicy,
    DurableMemorySession, EmbeddingExecutorConfig, EmbeddingProvider,
};
use aster_durable_memory::repository::{
    DurableMemoryKind, EvidenceKind, EvidenceRef, InMemoryRepository, MemoryChangeSet,
    MemoryNamespace, MemoryNodeDraft, MemoryOperation, MemoryRepository, MemoryStatus,
};
use aster_durable_memory::vector::{InMemoryVectorIndex, VectorIndexDescriptor};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::path::PathBuf;
use std::process::Command;
use std::sync::Arc;
use tokio_util::sync::CancellationToken;

#[tokio::test]
async fn frozen_memory_owner_uses_the_existing_repository_and_bundled_provider() {
    let raw_fixture = include_bytes!("../fixtures/retrieval_qualification_v1.json");
    let fixture_digest = format!("{:x}", Sha256::digest(raw_fixture));
    assert_eq!(
        fixture_digest,
        "9dd4e0b0d6d17a91aba31932e346c9bf72cee69e8a0e65cf242f5cfe154d8b19"
    );
    let fixture: Value = serde_json::from_slice(raw_fixture).unwrap();
    let documents = fixture["documents"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|document| document["owner"] == "memory")
        .cloned()
        .collect::<Vec<_>>();
    let queries = fixture["queries"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|query| query["owner"] == "memory")
        .collect::<Vec<_>>();
    assert_eq!(documents.len(), 10);
    assert_eq!(queries.len(), 10);
    let provider = Arc::new(aster_embedding::LightTierProvider::load().unwrap());
    let descriptor = provider.descriptor();
    let repository = Arc::new(InMemoryRepository::new());
    let namespace =
        MemoryNamespace::try_new("fixture", "principal", "frozen-memory-owner").unwrap();
    let time = chrono::DateTime::from_timestamp(1_777_000_000, 0).unwrap();
    let operations = documents
        .iter()
        .map(|document| {
            let id = document["id"].as_str().unwrap();
            let content = document["content"].as_str().unwrap();
            assert_eq!(
                format!("{:x}", Sha256::digest(content.as_bytes())),
                document["sha256"]
            );
            MemoryOperation::Create {
                node: MemoryNodeDraft::new(
                    id,
                    namespace.clone(),
                    DurableMemoryKind::Procedural,
                    MemoryStatus::Active,
                    content,
                    vec![EvidenceRef::try_new(
                        format!("aster://fixture/frozen-memory-owner/{id}"),
                        format!("sha256:{}", document["sha256"].as_str().unwrap()),
                        EvidenceKind::Verification,
                        time,
                    )
                    .unwrap()],
                    time,
                ),
            }
        })
        .collect();
    repository
        .apply(MemoryChangeSet::new(
            "seed-frozen-memory-owner",
            namespace.clone(),
            time,
            operations,
        ))
        .await
        .unwrap();

    let mut profiles = Vec::new();
    let max_results = fixture["output_limits"]["host_hits"].as_u64().unwrap() as usize;
    for linear in [false, true] {
        let policy = DurableMemorySemanticRecallPolicy::try_new(10, 0.0).unwrap();
        let semantic = DurableMemorySemanticRecall::new(
            format!("sha256:{}", "a".repeat(64)),
            provider.clone(),
            EmbeddingExecutorConfig::default(),
            Arc::new(
                InMemoryVectorIndex::new(VectorIndexDescriptor::new(descriptor.dimension)).unwrap(),
            ),
            if linear {
                policy.with_linear_fusion()
            } else {
                policy
            },
        )
        .unwrap();
        let binding = semantic.binding().clone();
        let session = DurableMemorySession::active_recall(
            repository.clone(),
            namespace.clone(),
            DurableMemoryRecallPolicy::try_new(max_results, 0.0).unwrap(),
        )
        .with_semantic_recall(semantic)
        .unwrap();
        let refresh = session
            .refresh_semantic_recall(CancellationToken::new())
            .await
            .unwrap();
        assert_eq!(refresh.active_node_count(), 10);
        let mut results = serde_json::Map::new();
        for query in &queries {
            let started = std::time::Instant::now();
            let preview = session
                .preview_recall(query["query"].as_str().unwrap())
                .await
                .unwrap();
            let elapsed = started.elapsed().as_secs_f64() * 1000.0;
            for hit in &preview.hits {
                let current = repository
                    .get(&namespace, &hit.node_id)
                    .await
                    .unwrap()
                    .unwrap();
                assert_eq!(current.status, MemoryStatus::Active);
                assert_eq!(current.revision, hit.node_revision);
                assert_eq!(current.content, hit.content);
            }
            results.insert(
                query["id"].as_str().unwrap().into(),
                json!({
                    "query": query["query"], "hits": preview.hits, "elapsed_ms": elapsed,
                }),
            );
        }
        profiles.push(json!({"binding": binding, "queries": results}));
    }
    let report = json!({
        "schema_version": 1, "owner": "core-v2-active-recall", "fixture_sha256": fixture_digest,
        "provider": descriptor, "documents": documents, "profiles": profiles,
        "recall_policy": {"max_results": max_results, "min_lexical_score": 0.0},
    });
    let temporary = tempfile::Builder::new()
        .prefix("memory qualification 中文 ")
        .tempdir()
        .unwrap();
    let directory = std::env::var_os("ASTER_RETRIEVAL_EVIDENCE_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| temporary.path().to_path_buf());
    std::fs::create_dir_all(&directory).unwrap();
    let raw = directory.join("core-memory-owner-raw.json");
    std::fs::write(&raw, serde_json::to_vec_pretty(&report).unwrap()).unwrap();
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../..");
    let output = Command::new(super::python::executable())
        .arg(root.join("tests/retrieval_bench/memory_quality.py"))
        .arg("--fixture")
        .arg(root.join("crates/code/acp/tests/fixtures/retrieval_qualification_v1.json"))
        .arg("--raw")
        .arg(raw)
        .arg("--output")
        .arg(directory.join("core-memory-owner-quality.json"))
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "actual memory qualification failed: {}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}
