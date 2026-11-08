//! Crate-level proofs: closed sets, wire limits, manifest validation,
//! numeric contracts, and fail-closed readiness.

use crate::assets::{bge_status, Family};
use crate::install::{InstallManifest, ManifestFile};
use crate::math::{l2_normalize, sigmoid};
use crate::wire::{EmbeddingRequest, RerankRequest};

#[test]
fn family_parse_names_the_closed_set() {
    assert_eq!(Family::parse("bge-m3").unwrap(), Family::BgeM3);
    assert_eq!(
        Family::parse(" bge-reranker-v2-m3 ").unwrap(),
        Family::BgeRerankerV2M3
    );
    let error = Family::parse("bge-m4").unwrap_err();
    assert_eq!(error.code, "use.bge.family_invalid");
    assert!(error.message.contains("bge-m3"));
    assert!(error.message.contains("bge-reranker-v2-m3"));
}

#[test]
fn embedding_input_accepts_string_or_array() {
    let one = EmbeddingRequest::from_json(br#"{"input":"hello"}"#).unwrap();
    assert_eq!(one.input, vec!["hello".to_string()]);
    let many = EmbeddingRequest::from_json(br#"{"model":"m","input":["a","b"]}"#).unwrap();
    assert_eq!(many.input.len(), 2);
    assert_eq!(many.model.as_deref(), Some("m"));
}

#[test]
fn embedding_input_refuses_token_arrays_and_other_shapes() {
    let error = EmbeddingRequest::from_json(br#"{"input":[[1,2]]}"#).unwrap_err();
    assert!(error.message.contains("must be a string"));
    let error = EmbeddingRequest::from_json(br#"{"input":42}"#).unwrap_err();
    assert!(error.message.contains("array of strings"));
    let error = EmbeddingRequest::from_json(br#"{"input":[]}"#).unwrap_err();
    assert!(error.message.contains("1-256"));
}

#[test]
fn embedding_refuses_non_float_encoding_format() {
    let error =
        EmbeddingRequest::from_json(br#"{"input":"a","encoding_format":"base64"}"#).unwrap_err();
    assert!(error.message.contains("float"));
}

#[test]
fn rerank_top_n_is_bounded_by_documents() {
    let error =
        RerankRequest::from_json(br#"{"query":"q","documents":["a"],"top_n":2}"#).unwrap_err();
    assert!(error.message.contains("must not exceed"));
    let request =
        RerankRequest::from_json(br#"{"query":"q","documents":["a","b"],"top_n":1}"#).unwrap();
    assert_eq!(request.effective_top_n(), 1);
    let request = RerankRequest::from_json(br#"{"query":"q","documents":["a","b"]}"#).unwrap();
    assert_eq!(request.effective_top_n(), 2);
}

#[test]
fn rerank_refuses_empty_documents_and_bad_top_n() {
    let error = RerankRequest::from_json(br#"{"query":"q","documents":[]}"#).unwrap_err();
    assert!(error.message.contains("1-256"));
    let error =
        RerankRequest::from_json(br#"{"query":"q","documents":["a"],"top_n":-1}"#).unwrap_err();
    assert!(error.message.contains("top_n"));
}

fn file(path: &str, url: &str, bytes: u64, sha256: &str) -> ManifestFile {
    ManifestFile {
        path: path.to_string(),
        url: url.to_string(),
        bytes,
        sha256: sha256.to_string(),
    }
}

fn sha() -> String {
    format!("{}ff", "a".repeat(62))
}

fn base_manifest() -> InstallManifest {
    InstallManifest::new(
        Family::BgeM3,
        "00000000-0000-4000-8000-000000000001",
        "http://127.0.0.1:3000",
        "onnx/model_quantized.onnx",
        "tokenizer.json",
        vec![
            file(
                "onnx/model_quantized.onnx",
                "http://127.0.0.1:9000/models/x/onnx/model_quantized.onnx",
                1024,
                &sha(),
            ),
            file(
                "tokenizer.json",
                "http://127.0.0.1:9000/models/x/tokenizer.json",
                2048,
                &sha(),
            ),
        ],
    )
    .unwrap()
}

#[test]
fn manifest_accepts_presigned_http_and_names_roles() {
    let manifest = base_manifest();
    assert_eq!(manifest.family().unwrap(), Family::BgeM3);
}

#[test]
fn manifest_refuses_traversal_and_absolute_paths() {
    let mut manifest = base_manifest();
    manifest.files[0].path = "../escape.onnx".to_string();
    let error = manifest.validate().unwrap_err();
    assert!(error.message.contains("relative path"));

    let mut manifest = base_manifest();
    manifest.files[0].path = "/absolute.onnx".to_string();
    assert!(manifest.validate().is_err());
}

#[test]
fn manifest_refuses_unlisted_roles_and_duplicate_paths() {
    let mut manifest = base_manifest();
    manifest.model_file = "onnx/other.onnx".to_string();
    let error = manifest.validate().unwrap_err();
    assert!(error.message.contains("not present in the manifest files"));

    let mut manifest = base_manifest();
    manifest.files[1].path = manifest.files[0].path.clone();
    let error = manifest.validate().unwrap_err();
    assert!(error.message.contains("repeats path"));
}

#[test]
fn manifest_refuses_credentials_and_bad_hashes() {
    let mut manifest = base_manifest();
    manifest.files[0].url = "http://user:pass@127.0.0.1:9000/models/models/x".to_string();
    let error = manifest.validate().unwrap_err();
    assert!(error.message.contains("credential-free"));

    let mut manifest = base_manifest();
    manifest.files[0].sha256 = sha().to_ascii_uppercase();
    let error = manifest.validate().unwrap_err();
    assert!(error.message.contains("lowercase hex"));
}

#[test]
fn cls_pooling_normalizes_and_sigmoid_maps_logit_to_score() {
    let mut vector = [3.0, 4.0];
    l2_normalize(&mut vector);
    assert!((vector[0] - 0.6).abs() < 1e-6);
    assert!((vector[1] - 0.8).abs() < 1e-6);
    let norm: f32 = vector.iter().map(|value| value * value).sum::<f32>().sqrt();
    assert!((norm - 1.0).abs() < 1e-6);

    assert!((sigmoid(0.0) - 0.5).abs() < 1e-6);
    assert!(sigmoid(-40.0) > 0.0);
    // f32 saturates toward 1.0 for large logits; assert a finite mid-range
    // value instead of a strict inequality that rounding can break.
    assert!((sigmoid(5.0) - 0.99330715).abs() < 1e-6);
    assert!((sigmoid(40.0) - 1.0).abs() < 1e-6);
}

#[test]
fn engines_report_fail_closed_readiness_by_name() {
    // Without an install the status must name the missing model; with a
    // real install the status must point at an existing directory. Both
    // branches assert the truth of the machine they run on.
    for family in [Family::BgeM3, Family::BgeRerankerV2M3] {
        let status = bge_status(family);
        if status.available {
            assert!(
                status.model_dir.is_some_and(|dir| dir.is_dir()),
                "{family:?} reports ready without a model directory"
            );
        } else {
            assert!(
                status.detail.contains("not installed")
                    || status.detail.contains("receipt")
                    || status.detail.contains("asset"),
                "{family:?} missing-status must name its reason: {}",
                status.detail
            );
        }
    }
}

#[cfg(feature = "bge-m3")]
#[tokio::test]
async fn embedding_engine_fails_closed_without_weights() {
    let client = crate::EmbeddingClient::from_env().unwrap();
    let status = client.status();
    if !status.available {
        let error = crate::EmbeddingClient::from_env()
            .unwrap()
            .embed(EmbeddingRequest::from_json(br#"{"input":["hello"]}"#).unwrap())
            .await
            .expect_err("served without weights");
        assert!(
            error.code == "use.bge.model_missing" || error.code == "use.bge.model_invalid",
            "unexpected error code: {error_code}",
            error_code = error.code
        );
    }
}

#[cfg(feature = "bge-reranker-v2-m3")]
#[tokio::test]
async fn rerank_engine_fails_closed_without_weights() {
    let client = crate::RerankClient::from_env().unwrap();
    let status = client.status();
    if !status.available {
        let error = client
            .rerank(RerankRequest::from_json(br#"{"query":"q","documents":["a"]}"#).unwrap())
            .await
            .expect_err("served without weights");
        assert!(
            error.code == "use.bge.model_missing" || error.code == "use.bge.model_invalid",
            "unexpected error code: {error_code}",
            error_code = error.code
        );
    }
}
