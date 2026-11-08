//! Explicit native Power composition. Both host inputs are private ACL files.
use aster_deepseek::{
    backend::NativeXqBackend,
    execution::XqPipelinePlan,
    hosting::{read_private_acl, FileLeaseAuth, NativeServingConfig},
    model::DeepSeekModel,
    parallel::XqPlacement,
    prompt::XqTextCodec,
    replicas::XqReplicaPool,
    weights::{
        engram::VerifiedEngramTables,
        xq::{gguf::XQ_GGUF_BYTES, storage::XqWeights, XQ_GGUF_SHA256},
    },
};
use aster_power::{
    backend::Backend,
    config::PowerConfig,
    error::{PowerError, Result},
    inference::{InferenceLimits, VerifiedRawWeightFile, WeightFileDescriptor},
    model::manifest::{ModelFormat, ModelManifest, ModelParameters},
    server::PowerServerBuilder,
};
use std::{
    path::PathBuf,
    sync::Arc,
    time::{Duration, Instant},
};
use tokio_util::sync::CancellationToken;
const GIB: u64 = 1024 * 1024 * 1024;

#[tokio::main(flavor = "multi_thread", worker_threads = 4)]
async fn main() -> Result<()> {
    if !cfg!(feature = "cuda") {
        return Err(PowerError::BackendNotAvailable(
            "native serving requires CUDA; CPU fallback is forbidden".into(),
        ));
    }
    let arguments: Vec<_> = std::env::args().skip(1).collect();
    if arguments.len() != 2 {
        return Err(PowerError::Config(
            "expected private Power ACL and native serving ACL paths".into(),
        ));
    }
    let power_path = PathBuf::from(&arguments[0]);
    read_private_acl(&power_path, 16384)?;
    let config = PowerConfig::load_from(&arguments[0])?;
    if !config.api_keys.is_empty()
        || !config.proxy_upstreams.is_empty()
        || config.num_parallel != 1
        || config.flash_attention
        || config.use_mlock
        || config.num_thread.is_some()
        || config.spec_mode != "off"
        || config.keep_alive != "-1"
        || config.max_concurrent_requests == 0
        || config.max_concurrent_requests > 16
        || config.max_queued_requests > 64
    {
        return Err(PowerError::Config("native composition requires host lease authentication, local-only registry, serial per-replica requests, spec off, keep_alive -1 and bounded front admission".into()));
    }
    let hosting =
        NativeServingConfig::from_acl(&read_private_acl(&PathBuf::from(&arguments[1]), 16384)?)?;
    let auth = Arc::new(FileLeaseAuth::new(hosting.auth_root.clone())?);
    let started = Instant::now();
    let cancellation = CancellationToken::new();
    let limits = InferenceLimits {
        max_model_bytes: 512 * GIB,
        max_resident_weight_bytes: 100 * GIB,
        max_state_bytes: 32 * GIB,
        max_tensor_elements: 16 * 1024 * 1024 * 1024,
        max_context_tokens: hosting.context_tokens,
        max_generated_tokens: hosting.max_generated_tokens,
        max_concurrent_requests: 1,
        ..InferenceLimits::default()
    };
    limits.validate()?;
    println!(
        "{}",
        serde_json::json!({"phase":"verifying_shared_native_sources"})
    );
    let weights = Arc::new(XqWeights::from_verified_file(
        VerifiedRawWeightFile::open(
            &hosting.payload_root,
            WeightFileDescriptor {
                relative_path: "V41-XQ-1017-v3.gguf".into(),
                bytes: XQ_GGUF_BYTES,
                sha256: XQ_GGUF_SHA256.into(),
            },
            &limits,
            16 * GIB,
            &cancellation,
        )?,
        &cancellation,
    )?);
    let mut files = Vec::new();
    for (bytes, sha256) in [
        (
            101535150936,
            "76a05106ece259be83d1bfea0dcb54f23377864ee60fd321690f35e6698a2af5",
        ),
        (
            101537926640,
            "9dc51433e195bc78b5ad846b68643cc0351d3b174e5e4309222d3b7402c83afd",
        ),
    ] {
        files.push(VerifiedRawWeightFile::open(
            &hosting.official_blob_root,
            WeightFileDescriptor {
                relative_path: sha256.into(),
                bytes,
                sha256: sha256.into(),
            },
            &limits,
            16 * 1024 * 1024,
            &cancellation,
        )?);
    }
    let tables = Arc::new(VerifiedEngramTables::from_verified_files(
        files
            .try_into()
            .map_err(|_| PowerError::Config("Engram source count changed".into()))?,
        &cancellation,
    )?);
    let tokenizer = VerifiedRawWeightFile::open(
        &hosting.official_blob_root,
        WeightFileDescriptor {
            relative_path: hosting.tokenizer_file.clone(),
            bytes: hosting.tokenizer_bytes,
            sha256: aster_deepseek::tokenizer::OFFICIAL_TOKENIZER_SHA256.into(),
        },
        &limits,
        16 * 1024 * 1024,
        &cancellation,
    )?;
    let bytes = tokenizer.read_range(0, hosting.tokenizer_bytes, &cancellation)?;
    let codec = Arc::new(XqTextCodec::from_official_bytes(bytes.bytes())?);
    println!(
        "{}",
        serde_json::json!({"phase":"loading_native_replica_pool","layout":hosting.layout.as_str(),"elapsed_seconds":started.elapsed().as_secs_f64()})
    );
    let plan = XqPipelinePlan {
        placement: XqPlacement::balanced(hosting.layout.width()),
        context_tokens: hosting.context_tokens,
        max_step_tokens: 8,
        max_peer_bytes: GIB,
        max_peer_copies: 256,
        max_staged_bytes: 8 * GIB,
        driver_reserve_bytes: 4 * GIB,
    };
    let pool = Arc::new(XqReplicaPool::load(
        weights,
        tables,
        hosting.layout,
        plan,
        limits,
        hosting.capacity_per_replica,
        Duration::from_secs(hosting.load_seconds),
    )?);
    println!(
        "{}",
        serde_json::json!({"phase":"native_replicas_loaded","admissions":pool.admissions(),"elapsed_seconds":started.elapsed().as_secs_f64()})
    );
    let model = DeepSeekModel::V41Flash1017Xq;
    let manifest = ModelManifest {
        name: model.served_alias().into(),
        format: ModelFormat::Gguf,
        size: XQ_GGUF_BYTES,
        sha256: XQ_GGUF_SHA256.into(),
        parameters: Some(ModelParameters {
            context_length: Some(hosting.context_tokens as u32),
            embedding_length: None,
            parameter_count: None,
            quantization: Some("XQ-1017-v3".into()),
        }),
        created_at: chrono::Utc::now(),
        path: hosting.payload_root.join("V41-XQ-1017-v3.gguf"),
        system_prompt: None,
        template_override: None,
        default_parameters: None,
        modelfile_content: None,
        license: Some("MIT".into()),
        adapter_path: None,
        adapter_artifact: None,
        external_draft: None,
        projector_path: None,
        projector_artifact: None,
        messages: Vec::new(),
        family: Some("deepseek41".into()),
        families: None,
    };
    let backend = Arc::new(NativeXqBackend::new(
        pool.clone(),
        codec,
        manifest.path.clone(),
        hosting.mode,
        hosting.effort,
    )?);
    backend.load(&manifest).await?;
    let result = PowerServerBuilder::new(config)
        .without_default_backends()
        .with_backend(backend)
        .with_model_manifest(manifest)
        .with_auth_provider(auth)
        .start()
        .await;
    let drain = pool.shutdown(Duration::from_secs(30));
    result?;
    drain
}
