//! Verified official parameters -> callable native layer components. This
//! composition keeps raw low-precision matrices and source-only projections;
//! it neither registers a backend nor substitutes an external model executor.

use aster_power::error::Result;
#[cfg(test)]
use candle_core::Tensor;
use tokio_util::sync::CancellationToken;

use super::{
    check_cancel, invalid, ExpertBindings, LayerWeightSchema, MatrixBinding, MhcBindings,
    ParameterBinding,
};
use crate::{
    layers::{
        attention::{
            branch::NativeAttention, output::AttentionOutputProjection,
            projection::AttentionInputProjection, sublayer::AttentionSublayer,
        },
        compressor::KvCompressor,
        decoder::TextDecoderLayer,
        engram::{lookup::EngramTable, projection::EngramProjection, EngramGate},
        indexer::{
            projection::{IndexKeyProjection, IndexQueryProjection},
            CandidateBlockPolicy, CandidateBlockWidth, SparseIndexer,
        },
        kv::MainKvFormat,
        linear::{expert::QuantizedExpert, DeviceMatrix},
        mhc::{MhcBlock, MhcNumerics, MhcParameters},
        moe::{block::MoeBlock, router::MoeRouter},
        norm::EffectiveRmsNorm,
        sublayer::MoeSublayer,
        CudaLayerDevice,
    },
    model::topology::{LayerBinding, TextArchitecture},
    weights::CheckpointWeights,
};

pub struct LoadedTextLayer<'a> {
    device: CudaLayerDevice,
    binding: LayerBinding,
    pub decoder: TextDecoderLayer,
    pub compressor: Option<KvCompressor>,
    pub index_query: Option<IndexQueryProjection>,
    pub index_key: Option<IndexKeyProjection>,
    pub indexer: Option<SparseIndexer>,
    pub engram_table: Option<EngramTable<'a>>,
    resident_upper_bytes: u64,
}

impl<'a> LoadedTextLayer<'a> {
    /// Public entry always reconstructs the pinned schema from the verified
    /// architecture. An edited schema cannot activate an official native layer.
    pub fn load(
        checkpoint: &'a CheckpointWeights,
        architecture: &TextArchitecture,
        layer: usize,
        device: &CudaLayerDevice,
        context: usize,
        format: MainKvFormat,
        cancellation: &CancellationToken,
    ) -> Result<Self> {
        let binding = architecture.layer(layer)?;
        load_schema(
            checkpoint,
            &LayerWeightSchema::new(binding),
            binding,
            architecture.mhc_numerics(),
            device,
            context,
            format,
            cancellation,
        )
    }
    pub fn binding(&self) -> &LayerBinding {
        &self.binding
    }
    /// Conservative component/storage bound, not a model-wide Power permit
    /// or an observed allocator peak. Huge Engram tables remain range-backed.
    pub fn resident_upper_bytes(&self) -> u64 {
        self.resident_upper_bytes
    }
}

fn load_schema<'a>(
    checkpoint: &'a CheckpointWeights,
    schema: &LayerWeightSchema,
    binding: &LayerBinding,
    numerics: MhcNumerics,
    device: &CudaLayerDevice,
    context: usize,
    format: MainKvFormat,
    cancellation: &CancellationToken,
) -> Result<LoadedTextLayer<'a>> {
    check_cancel(cancellation)?;
    if binding.compression_numerics() != crate::state::CompressionNumerics::OfficialBf16 {
        return Err(invalid(
            "official text loader requires its bound official BF16 architecture",
        ));
    }
    if schema.layer() != binding.layer()
        || schema.compressor.is_some() != binding.is_kv_source()
        || schema.index_query.is_some() != binding.is_index_topk_source()
        || schema.index_key.is_some() != binding.is_index_k_source()
        || schema.engram.is_some() != binding.engram().is_some()
    {
        return Err(invalid(
            "layer schema differs from its exact topology owners",
        ));
    }
    schema.validate(checkpoint, cancellation)?;
    let resident_upper_bytes = resident_bound(schema, context)?;
    if resident_upper_bytes > device.limits().max_resident_weight_bytes {
        return Err(invalid(
            "complete layer exceeds its pre-upload resident byte bound",
        ));
    }
    let cache = binding.new_rope_cache(device, context, cancellation)?;
    let cos_sin = cache.cos_sin();
    let a = &schema.attention;
    let load = |matrix: &MatrixBinding| load_matrix(checkpoint, matrix, device, cancellation);
    let norm =
        |parameter: &ParameterBinding| load_norm(checkpoint, parameter, device, cancellation);
    let mhc =
        |parameters: &MhcBindings| load_mhc(checkpoint, parameters, numerics, device, cancellation);
    let input = AttentionInputProjection::new(
        device.clone(),
        load(&a.query_a)?,
        load(&a.query_b)?,
        load(&a.window_kv)?,
        norm(&a.query_norm)?,
        norm(&a.kv_norm)?,
        cos_sin.clone(),
    )?;
    let grouped_width = a.output_a.logical_shape()[1];
    let query_width = a.query_b.logical_shape()[0];
    if grouped_width == 0 || !query_width.is_multiple_of(grouped_width) {
        return Err(invalid(
            "declared grouped attention projection geometry differs",
        ));
    }
    let output = AttentionOutputProjection::new(
        device.clone(),
        query_width / grouped_width,
        load(&a.output_a)?,
        load(&a.output_b)?,
        cos_sin.clone(),
    )?;
    let attention = AttentionSublayer::new(
        device.clone(),
        mhc(&a.mhc)?,
        norm(&a.norm)?,
        NativeAttention::new(
            input,
            output,
            a.sink.load(checkpoint, device, cancellation)?,
        )?,
    )?;
    let f = &schema.ffn;
    // The BF16 gate is a small unquantized parameter, unlike the expert
    // matrices; its FP32 logits and sqrt-softplus stay in MoeRouter's owner.
    f.router.bind(checkpoint)?;
    let gate = ParameterBinding::new(f.router.weight_name(), "bf16", &f.router.logical_shape())
        .load(checkpoint, device, cancellation)?;
    let router = MoeRouter::new(
        device.clone(),
        gate,
        f.router_bias.load(checkpoint, device, cancellation)?,
        f.image_router_bias.load(checkpoint, device, cancellation)?,
        6,
        1.5,
    )?;
    let expert = |e: &ExpertBindings| {
        QuantizedExpert::new(
            device.clone(),
            load(&e.gate)?,
            load(&e.up)?,
            load(&e.down)?,
            10.,
        )
    };
    let routed = f.routed.iter().map(expert).collect::<Result<Vec<_>>>()?;
    let ffn = MoeSublayer::new(
        device.clone(),
        mhc(&f.mhc)?,
        norm(&f.norm)?,
        MoeBlock::new(device.clone(), router, routed, expert(&f.shared)?)?,
    )?;
    let engram = schema
        .engram
        .as_ref()
        .map(|e| {
            let gate = EngramGate::new(
                device.clone(),
                e.query.load(checkpoint, device, cancellation)?,
                e.key.load(checkpoint, device, cancellation)?,
                1e-20,
                1e-6,
            )?;
            EngramProjection::new(device.clone(), load(&e.projection)?, gate)
        })
        .transpose()?;
    let decoder = TextDecoderLayer::new(device.clone(), binding, attention, ffn, engram)?;
    let compressor = schema
        .compressor
        .as_ref()
        .map(|c| {
            let kind = binding
                .compression()
                .ok_or_else(|| invalid("source compressor is missing its declared kind"))?;
            KvCompressor::new(
                device.clone(),
                kind,
                load(&c.value)?,
                c.gate.as_ref().map(load).transpose()?,
                norm(&c.norm)?,
                cos_sin.clone(),
                format,
            )
        })
        .transpose()?;
    let index_query = schema
        .index_query
        .as_ref()
        .map(|i| {
            IndexQueryProjection::new(
                device.clone(),
                load(&i.query)?,
                load(&i.head_weights)?,
                cos_sin.clone(),
            )
        })
        .transpose()?;
    let index_key = schema
        .index_key
        .as_ref()
        .map(|i| {
            IndexKeyProjection::new(
                device.clone(),
                load(&i.key)?,
                norm(&i.norm)?,
                cos_sin.clone(),
            )
        })
        .transpose()?;
    let indexer = schema
        .index_query
        .as_ref()
        .map(|i| {
            let heads = i.head_weights.logical_shape()[0];
            let policy = binding
                .is_candidate_source()
                .then(|| CandidateBlockPolicy::new(CandidateBlockWidth::Eight, 2048))
                .transpose()?;
            SparseIndexer::new(device.clone(), heads, 128, 512, policy)
        })
        .transpose()?;
    let engram_table = schema
        .engram
        .as_ref()
        .map(|e| EngramTable::new(device.clone(), e.table.bind(checkpoint)?))
        .transpose()?;
    check_cancel(cancellation)?;
    Ok(LoadedTextLayer {
        device: device.clone(),
        binding: binding.clone(),
        decoder,
        compressor,
        index_query,
        index_key,
        indexer,
        engram_table,
        resident_upper_bytes,
    })
}

fn load_matrix(
    checkpoint: &CheckpointWeights,
    binding: &MatrixBinding,
    device: &CudaLayerDevice,
    cancellation: &CancellationToken,
) -> Result<DeviceMatrix> {
    check_cancel(cancellation)?;
    let weights = binding.bind(checkpoint)?;
    DeviceMatrix::load(
        device,
        &weights,
        0,
        binding.logical_shape()[0],
        cancellation,
    )
}
fn load_norm(
    checkpoint: &CheckpointWeights,
    binding: &ParameterBinding,
    device: &CudaLayerDevice,
    cancellation: &CancellationToken,
) -> Result<EffectiveRmsNorm> {
    if binding.shape().len() != 1 {
        return Err(invalid("effective norm binding must be a declared vector"));
    }
    EffectiveRmsNorm::new(
        device.clone(),
        binding.shape()[0],
        1e-20,
        Some(binding.load(checkpoint, device, cancellation)?),
    )
}
fn load_mhc(
    checkpoint: &CheckpointWeights,
    binding: &MhcBindings,
    numerics: MhcNumerics,
    device: &CudaLayerDevice,
    cancellation: &CancellationToken,
) -> Result<MhcBlock> {
    MhcBlock::new(
        device.clone(),
        MhcParameters {
            norm_fused_projection: binding.projection.load(checkpoint, device, cancellation)?,
            scale: binding.scale.load(checkpoint, device, cancellation)?,
            base: binding.base.load(checkpoint, device, cancellation)?,
        },
        numerics,
    )
}

fn resident_bound(schema: &LayerWeightSchema, context: usize) -> Result<u64> {
    let table_names = schema
        .engram
        .as_ref()
        .map(|e| e.table.tensors())
        .unwrap_or_default();
    let mut raw = 0_u64;
    for tensor in schema.tensors() {
        if !table_names
            .iter()
            .any(|table| table.name() == tensor.name())
        {
            raw = raw
                .checked_add(tensor.bytes())
                .ok_or_else(|| invalid("layer storage byte sum overflowed"))?;
        }
    }
    // Twice raw storage covers FP8 row maps, effective F32 norm snapshots and
    // parameter ownership. Up to five separate width-64 rotary caches cover
    // attention input/output, compression, index Q/K, plus frequency metadata.
    raw.checked_mul(2)
        .and_then(|n| {
            (context as u64)
                .checked_mul(64 * 4 * 5)
                .and_then(|c| n.checked_add(c))
        })
        .and_then(|n| n.checked_add(4096))
        .ok_or_else(|| invalid("layer resident upper byte bound overflowed"))
}

mod step;
#[cfg(test)]
mod tests;
