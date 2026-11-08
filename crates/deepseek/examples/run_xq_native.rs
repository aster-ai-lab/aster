//! Bounded full-checkpoint qualification; no HTTP reference or serving fallback.
//! Execution limits are fixed here. Arguments identify immutable model data,
//! a declared GPU-group width and raw qualification token IDs, not a product
//! configuration file. Production settings belong to the ACL service owner.

use std::{path::PathBuf, time::Instant};

use aster_deepseek::{
    execution::{XqNativePipeline, XqPipelinePlan},
    parallel::{XqPlacement, XqReplicaWidth},
    weights::{
        engram::{VerifiedEngramTables, XqEngramTableSource},
        xq::{gguf::XQ_GGUF_BYTES, storage::XqWeights, XQ_GGUF_SHA256},
    },
};
use aster_power::inference::{
    DevicePreference, InferenceLimits, RuntimeDevice, VerifiedRawWeightFile, WeightFileDescriptor,
};
use tokio_util::sync::CancellationToken;

const GIB: u64 = 1024 * 1024 * 1024;
const RAW_READ_BYTES: u64 = 16 * GIB;

#[path = "support/native_barrier.rs"]
mod native_barrier;

fn phase(name: &str, start: &Instant) {
    println!(
        "{}",
        serde_json::json!({"phase":name,"elapsed_seconds":start.elapsed().as_secs_f64()})
    );
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let arguments: Vec<_> = std::env::args().skip(1).collect();
    if arguments.len() != 5 && arguments.len() != 6 {
        return Err("expected payload-root, official-blob-root, group-width, comma-separated raw token IDs, maximum new tokens, and optional owned benchmark barrier directory".into());
    }
    let payload_root = PathBuf::from(&arguments[0]);
    let blob_root = PathBuf::from(&arguments[1]);
    let width = XqReplicaWidth::parse(&arguments[2])?;
    if arguments[3].len() > 4096 {
        return Err("raw qualification prompt exceeds its byte bound".into());
    }
    let prompt = arguments[3]
        .split(',')
        .map(str::parse::<u32>)
        .collect::<Result<Vec<_>, _>>()?;
    let maximum = arguments[4].parse::<usize>()?;
    if prompt.is_empty()
        || prompt.len() > 128
        || maximum == 0
        || maximum > 32
        || prompt.iter().any(|&token| {
            token as usize >= aster_deepseek::tokenizer::RAW_VOCABULARY || token == 129264
        })
    {
        return Err(
            "qualification requires 1..128 base-text prompt IDs and 1..32 new tokens".into(),
        );
    }
    let start = Instant::now();
    let cancellation = CancellationToken::new();
    let limits = InferenceLimits {
        max_model_bytes: 512 * GIB,
        max_resident_weight_bytes: 100 * GIB,
        max_state_bytes: 32 * GIB,
        max_tensor_elements: 16 * 1024 * 1024 * 1024,
        max_context_tokens: 256,
        max_generated_tokens: 32,
        max_concurrent_requests: 1,
        ..InferenceLimits::default()
    };
    limits.validate()?;
    phase("verifying_complete_xq_payload", &start);
    let source = VerifiedRawWeightFile::open(
        &payload_root,
        WeightFileDescriptor {
            relative_path: "V41-XQ-1017-v3.gguf".into(),
            bytes: XQ_GGUF_BYTES,
            sha256: XQ_GGUF_SHA256.into(),
        },
        &limits,
        RAW_READ_BYTES,
        &cancellation,
    )?;
    let weights = XqWeights::from_verified_file(source, &cancellation)?;
    phase("verifying_complete_official_engram_sources", &start);
    let mut files = Vec::with_capacity(2);
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
            &blob_root,
            WeightFileDescriptor {
                relative_path: sha256.into(),
                bytes,
                sha256: sha256.into(),
            },
            &limits,
            16 * 1024 * 1024,
            &cancellation,
        )?);
        phase("verified_one_complete_engram_source", &start);
    }
    let files: [VerifiedRawWeightFile; 2] = files
        .try_into()
        .map_err(|_| "Engram source count changed")?;
    let tables = VerifiedEngramTables::from_verified_files(files, &cancellation)?;
    let runtimes = (0..width.device_count())
        .map(|ordinal| RuntimeDevice::resolve(DevicePreference::Cuda { ordinal }))
        .collect::<Result<Vec<_>, _>>()?;
    let plan = XqPipelinePlan {
        placement: XqPlacement::balanced(width),
        context_tokens: 256,
        max_step_tokens: 8,
        max_peer_bytes: GIB,
        max_peer_copies: 256,
        max_staged_bytes: 8 * GIB,
        driver_reserve_bytes: 4 * GIB,
    };
    phase("loading_all_forty_layers", &start);
    let pipeline = XqNativePipeline::load_with_table_source(
        &weights,
        XqEngramTableSource::VerifiedFiles(&tables),
        &runtimes,
        plan,
        limits,
        &cancellation,
    )?;
    let ready = serde_json::json!({"phase":"model_loaded","model_sha256":XQ_GGUF_SHA256,"device_admissions":pipeline.device_admissions(),"elapsed_seconds":start.elapsed().as_secs_f64()});
    println!("{ready}");
    if let Some(root) = arguments.get(5) {
        native_barrier::wait(PathBuf::from(root).as_path(), &ready)?;
    }
    let generation_start = Instant::now();
    let generated = pipeline.generate_greedy_streaming(&prompt, maximum, &cancellation, |token| {
        println!("{}",serde_json::json!({"phase":"generated_token","token_id":token,"generation_elapsed_seconds":generation_start.elapsed().as_secs_f64()}));
        Ok(())
    })?;
    println!(
        "{}",
        serde_json::json!({
            "phase":"generated", "model_sha256":XQ_GGUF_SHA256, "native_execution":true,
            "group_width":width.as_str(), "generation":generated,
            "generation_seconds":generation_start.elapsed().as_secs_f64(),
            "total_seconds":start.elapsed().as_secs_f64(),
            "quality_or_performance_qualification_claimed":false
        })
    );
    Ok(())
}
