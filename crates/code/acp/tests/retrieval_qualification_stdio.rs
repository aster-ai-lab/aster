//! Portable qualification through the actual ACP binary and its stdio API.

#[path = "support/binaries.rs"]
mod binaries;
#[path = "../../../../apps/aster/tests/support/python.rs"]
mod python;

use std::io::Read;
use std::path::PathBuf;
use std::process::Command;
use std::sync::Arc;

use serde_json::Value;
use sha2::{Digest, Sha256};

#[tokio::test]
async fn bundled_semantic_order_survives_the_existing_vector_index() {
    use aster_core::{
        EmbeddingExecutor, EmbeddingExecutorConfig, EmbeddingInput, EmbeddingProvider,
    };
    use aster_durable_memory::vector::{
        InMemoryVectorIndex, VectorIndex, VectorIndexDescriptor, VectorRecord, VectorSearchRequest,
    };
    use tokio_util::sync::CancellationToken;

    let provider = Arc::new(aster_embedding::LightTierProvider::load().unwrap());
    let descriptor = provider.descriptor();
    let executor = EmbeddingExecutor::new(provider, EmbeddingExecutorConfig::default()).unwrap();
    let execution = executor
        .embed(
            vec![
                EmbeddingInput::new("query", "parse the config file"),
                EmbeddingInput::new("settings", "read configuration settings"),
                EmbeddingInput::new("weather", "the weather is sunny today"),
            ],
            CancellationToken::new(),
        )
        .await
        .unwrap();
    let mut vectors = execution.vectors.into_iter();
    let query = vectors.next().unwrap();
    assert_eq!(query.id.as_ref(), "query");
    let index = InMemoryVectorIndex::new(VectorIndexDescriptor::new(descriptor.dimension)).unwrap();
    index
        .replace_partition(
            "observed-memory-inputs",
            vectors
                .map(|vector| VectorRecord::new(vector.id.to_string(), vector.values))
                .collect(),
        )
        .await
        .unwrap();
    let result = index
        .search(VectorSearchRequest::new(query.values, 2))
        .await
        .unwrap();
    if let Some(directory) = std::env::var_os("ASTER_RETRIEVAL_EVIDENCE_DIR") {
        let directory = PathBuf::from(directory);
        std::fs::create_dir_all(&directory).unwrap();
        std::fs::write(
            directory.join("direct-semantic.json"),
            serde_json::to_vec_pretty(&serde_json::json!({
                "query": "parse the config file",
                "provider": descriptor,
                "hits": result.hits.iter().map(|hit| serde_json::json!({
                    "id": hit.id, "score": hit.score,
                })).collect::<Vec<_>>(),
            }))
            .unwrap(),
        )
        .unwrap();
    }
    assert_eq!(result.hits.len(), 2);
    assert_eq!(result.hits[0].id, "settings");
    assert_eq!(result.hits[1].id, "weather");
}

#[tokio::test]
async fn bundled_provider_binds_to_the_existing_durable_memory_projection() {
    let preview = bundled_memory_probe(false).await;
    // Default RRF fusion deliberately ranks the hybrid `weather` node first;
    // linear fusion is what preserves the semantic margin (see
    // durable_memory::fusion_tests::linear_preserves_similarity_margin_and_later_verified_revision).
    // This case qualifies the binding itself: both active memories are recalled
    // through the shared bundled provider and the candidate stays excluded.
    assert_eq!(preview.hits.len(), 2);
    assert!(preview.hits.iter().any(|hit| hit.node_id == "settings"));
    assert!(preview.hits.iter().any(|hit| hit.node_id == "weather"));
    assert!(preview.hits.iter().all(|hit| hit.node_id != "candidate"));
}

#[tokio::test]
async fn bundled_linear_profile_preserves_the_semantic_margin() {
    let preview = bundled_memory_probe(true).await;
    assert_eq!(preview.hits[0].node_id, "settings");
}

async fn bundled_memory_probe(linear: bool) -> aster_core::DurableMemoryRecallPreview {
    use aster_core::{
        DurableMemoryRecallChannel, DurableMemoryRecallPolicy, DurableMemorySemanticRecall,
        DurableMemorySemanticRecallPolicy, DurableMemorySession, EmbeddingExecutorConfig,
        EmbeddingProvider,
    };
    use aster_durable_memory::repository::{
        DurableMemoryKind, EvidenceKind, EvidenceRef, InMemoryRepository, MemoryChangeSet,
        MemoryNamespace, MemoryNodeDraft, MemoryOperation, MemoryRepository, MemoryStatus,
    };
    use aster_durable_memory::vector::{InMemoryVectorIndex, VectorIndexDescriptor};
    use tokio_util::sync::CancellationToken;

    let provider = Arc::new(aster_embedding::LightTierProvider::load().unwrap());
    let descriptor = provider.descriptor();
    let index = Arc::new(
        InMemoryVectorIndex::new(VectorIndexDescriptor::new(descriptor.dimension)).unwrap(),
    );
    let semantic = DurableMemorySemanticRecall::new(
        format!("sha256:{}", "a".repeat(64)),
        provider,
        EmbeddingExecutorConfig::default(),
        index,
        if linear {
            DurableMemorySemanticRecallPolicy::try_new(10, 0.0)
                .unwrap()
                .with_linear_fusion()
        } else {
            DurableMemorySemanticRecallPolicy::try_new(10, 0.0).unwrap()
        },
    )
    .expect("the shared bundled provider must serve the existing semantic-memory mechanism");
    assert_eq!(semantic.binding().embedding(), &descriptor);
    assert_eq!(
        semantic.binding().fusion_profile(),
        if linear {
            aster_core::DURABLE_MEMORY_SEMANTIC_LINEAR_FUSION_PROFILE_V1
        } else {
            aster_core::DURABLE_MEMORY_SEMANTIC_FUSION_PROFILE_V1
        }
    );
    let repository = Arc::new(InMemoryRepository::new());
    let namespace = MemoryNamespace::try_new("fixture", "principal", "light-memory").unwrap();
    let time = chrono::DateTime::from_timestamp(1_777_000_000, 0).unwrap();
    let operations = [
        (
            "settings",
            MemoryStatus::Active,
            "read configuration settings",
        ),
        (
            "weather",
            MemoryStatus::Active,
            "the weather is sunny today",
        ),
        (
            "candidate",
            MemoryStatus::Candidate,
            "read configuration settings",
        ),
    ]
    .into_iter()
    .map(|(id, status, content)| MemoryOperation::Create {
        node: MemoryNodeDraft::new(
            id,
            namespace.clone(),
            DurableMemoryKind::Procedural,
            status,
            content,
            vec![EvidenceRef::try_new(
                format!("aster://fixture/light-memory/{id}"),
                format!("sha256:{:x}", Sha256::digest(content.as_bytes())),
                EvidenceKind::Verification,
                time,
            )
            .unwrap()],
            time,
        ),
    })
    .collect();
    repository
        .apply(MemoryChangeSet::new(
            "seed-light",
            namespace.clone(),
            time,
            operations,
        ))
        .await
        .unwrap();
    let session = DurableMemorySession::active_recall(
        repository.clone(),
        namespace.clone(),
        DurableMemoryRecallPolicy::try_new(2, 0.0).unwrap(),
    )
    .with_semantic_recall(semantic)
    .unwrap();
    let receipt = session
        .refresh_semantic_recall(CancellationToken::new())
        .await
        .unwrap();
    assert_eq!(receipt.active_node_count(), 2);
    assert_eq!(receipt.index_status().record_count, 2);
    let preview = session
        .preview_recall("parse the config file")
        .await
        .unwrap();
    if let Some(directory) = std::env::var_os("ASTER_RETRIEVAL_EVIDENCE_DIR") {
        let directory = PathBuf::from(directory);
        std::fs::create_dir_all(&directory).unwrap();
        std::fs::write(
            directory.join(if linear {
                "bundled-linear-memory-preview.json"
            } else {
                "bundled-memory-preview.json"
            }),
            serde_json::to_vec_pretty(&serde_json::json!({
                "query": "parse the config file",
                "provider": descriptor,
                "preview": &preview,
            }))
            .unwrap(),
        )
        .unwrap();
    }
    let settings = preview
        .hits
        .iter()
        .find(|hit| hit.node_id == "settings")
        .expect("the synonym query must recall the active configuration memory");
    assert_eq!(settings.node_revision, 1);
    assert!(matches!(
        settings.channel,
        DurableMemoryRecallChannel::Semantic | DurableMemoryRecallChannel::Hybrid
    ));
    assert!(preview.hits.iter().all(|hit| hit.node_id != "candidate"));
    assert_eq!(
        repository
            .get(&namespace, "candidate")
            .await
            .unwrap()
            .unwrap()
            .status,
        MemoryStatus::Candidate
    );
    preview
}

#[path = "retrieval_qualification_stdio/memory.rs"]
mod memory;

fn workspace_round_trip(semantic: bool, frozen_fixture: bool) {
    let root = tempfile::Builder::new()
        .prefix("aster retrieval 路径 ")
        .tempdir()
        .unwrap();
    let corpus = root.path().join("corpus with spaces");
    let script = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../../tests/retrieval_bench/run_quality.py");
    let binary = PathBuf::from(binaries::acp_binary());
    let mut command = Command::new(python::executable());
    command
        .arg(script)
        .arg("--engine")
        .arg(&binary)
        .arg("--corpus")
        .arg(&corpus);
    if semantic {
        command.arg("--semantic");
    }
    let fixture = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/retrieval_qualification_v1.json");
    if frozen_fixture {
        command.arg("--fixture").arg(&fixture);
    }
    let output = command.output().expect("run actual ACP qualification");
    let profile = match (semantic, frozen_fixture) {
        (false, false) => "c0",
        (true, false) => "c1",
        (false, true) => "c0-quality",
        (true, true) => "c1-quality",
    };
    if let Some(directory) = std::env::var_os("ASTER_RETRIEVAL_EVIDENCE_DIR") {
        let directory = PathBuf::from(directory);
        std::fs::create_dir_all(&directory).unwrap();
        std::fs::write(
            directory.join(format!("{profile}-stdio.json")),
            &output.stdout,
        )
        .unwrap();
        std::fs::write(
            directory.join(format!("{profile}-stdio.stderr.log")),
            &output.stderr,
        )
        .unwrap();
    }
    assert!(
        output.status.success(),
        "ACP driver failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let report: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(report["queries"], if frozen_fixture { 60 } else { 3 });
    assert_eq!(report["qualification"]["status"], "NOT_RUN");
    if frozen_fixture {
        let qualification = &report["qualification"];
        assert_eq!(qualification["workspace_quality_status"], "PASS");
        assert_eq!(qualification["executed_queries"], 40);
        assert_eq!(qualification["pending"].as_array().unwrap().len(), 20);
        assert_eq!(qualification["attribution_failures"], 0);
        let fixture_bytes = std::fs::read(&fixture).unwrap();
        assert_eq!(
            qualification["fixture_sha256"],
            format!("{:x}", Sha256::digest(&fixture_bytes))
        );
        assert_eq!(
            report["suites"][0]["per_query"].as_object().unwrap().len(),
            40
        );
    }
    let identity = &report["engine_identity"];
    assert_eq!(identity["protocol"], "acp-stdio");
    assert_eq!(identity["mode"], "hybrid");
    assert_eq!(identity["semantic_opt_in"], semantic);
    let mut source = std::fs::File::open(&binary).unwrap();
    let mut hash = Sha256::new();
    let mut buffer = [0_u8; 64 * 1024];
    loop {
        let count = source.read(&mut buffer).unwrap();
        if count == 0 {
            break;
        }
        hash.update(&buffer[..count]);
    }
    assert_eq!(identity["binary_sha256"], format!("{:x}", hash.finalize()));
    let observations = identity["observations"].as_array().unwrap();
    assert!(observations.len() >= 3);
    let hits = observations
        .iter()
        .flat_map(|observation| observation["metadata"]["results"].as_array().unwrap());
    if !frozen_fixture {
        assert!(
            hits.clone().any(|hit| {
                hit["path"] == "renderer.rs"
                    && hit["chunk_start_line"].as_u64().unwrap() <= 903
                    && hit["chunk_end_line"].as_u64().unwrap() >= 903
                    && hit["text"].as_str().unwrap().contains("paint_widget_twice")
                    && hit["digest_verified"] == true
            }),
            "the actual source evidence at renderer.rs:903 never reached ACP"
        );
    }
    let statuses = observations
        .iter()
        .map(|observation| &observation["metadata"]["semantic_status"]);
    if semantic {
        assert!(statuses.clone().any(|status| {
            status["phase"] == "ready"
                && status["indexedFiles"] == if frozen_fixture { 40 } else { 3 }
                && status["coverageBps"] == 10_000
                && status["model"]["provider"] == "light"
        }));
        assert!(hits.clone().any(|hit| {
            hit["channels"]
                .as_array()
                .unwrap()
                .iter()
                .any(|channel| channel["channel"] == "semantic")
        }));
    } else {
        assert!(statuses.clone().all(|status| status["phase"] == "disabled"));
    }
    assert!(
        report["suites"]
            .as_array()
            .unwrap()
            .iter()
            .any(|suite| suite["suite"] == "aster-acp-hybrid"),
        "stand-in rankings substituted for the requested executable"
    );
}

#[test]
fn actual_acp_model_free_workspace_search_returns_verified_source() {
    workspace_round_trip(false, false);
}

#[test]
fn actual_acp_bundled_semantic_workspace_search_returns_verified_source() {
    workspace_round_trip(true, false);
}

#[test]
fn frozen_model_free_quality_uses_actual_rankings_and_retains_unrun_owners() {
    workspace_round_trip(false, true);
}

#[test]
fn frozen_semantic_quality_uses_actual_rankings_and_retains_unrun_owners() {
    workspace_round_trip(true, true);
}

#[test]
fn actual_phase_traces_keep_request_owners_and_redact_payloads() {
    let script = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../../tests/retrieval_bench/phase_stdio.py");
    let output = Command::new(python::executable())
        .arg(script)
        .arg("--engine")
        .arg(binaries::acp_binary())
        .output()
        .expect("run actual phase qualification");
    if let Some(directory) = std::env::var_os("ASTER_RETRIEVAL_EVIDENCE_DIR") {
        let directory = PathBuf::from(directory);
        std::fs::create_dir_all(&directory).unwrap();
        std::fs::write(directory.join("phase-stdio.json"), &output.stdout).unwrap();
        std::fs::write(directory.join("phase-stdio.stderr.log"), &output.stderr).unwrap();
    }
    assert!(
        output.status.success(),
        "phase qualification failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let report: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(report["scope"], "actual-process-phase-contract");
    let profiles = report["profiles"].as_array().unwrap();
    assert_eq!(profiles.len(), 2);
    assert!(profiles.iter().all(|profile| profile["status"] == "PASS"));
}
