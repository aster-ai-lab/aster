//! Exact official base-text parameter bindings. Names and shapes are declared
//! by the model owner from checkpoint 1bae0e1088993b9b82de08f59de5be5a34e42170;
//! no suffix inspection chooses a dtype, quantizer or shared-state owner.
//! Building/validating a schema does not read large tensors or allocate HBM.

use std::collections::BTreeSet;

use aster_power::error::Result;
use candle_core::{DType, Tensor};
use tokio_util::sync::CancellationToken;

use super::{invalid, CheckpointWeights, MatrixLayout, MatrixWeights};
use crate::{
    layers::{tensor_error, validate_finite, CudaLayerDevice},
    model::topology::{LayerBinding, TextArchitecture},
    state::CompressionKind,
};

pub mod load;

#[derive(Debug, Clone)]
pub struct MatrixBinding {
    weight: String,
    scale: Option<String>,
    layout: MatrixLayout,
    shape: [usize; 2],
}

impl MatrixBinding {
    fn new(prefix: &str, layout: MatrixLayout, shape: [usize; 2]) -> Self {
        Self {
            weight: format!("{prefix}.weight"),
            scale: (!matches!(layout, MatrixLayout::Bf16 | MatrixLayout::F32))
                .then(|| format!("{prefix}.scale")),
            layout,
            shape,
        }
    }

    pub fn bind<'a>(&self, checkpoint: &'a CheckpointWeights) -> Result<MatrixWeights<'a>> {
        checkpoint.matrix(&self.weight, self.scale.as_deref(), self.layout, self.shape)
    }

    pub fn weight_name(&self) -> &str {
        &self.weight
    }
    pub fn layout(&self) -> MatrixLayout {
        self.layout
    }
    pub fn logical_shape(&self) -> [usize; 2] {
        self.shape
    }

    fn tensors(&self) -> Vec<ParameterBinding> {
        let [rows, columns] = self.shape;
        let (dtype, width) = match self.layout {
            MatrixLayout::Bf16 => ("bf16", columns),
            MatrixLayout::F32 => ("f32", columns),
            MatrixLayout::Mxfp4Block32 => ("i8", columns / 2),
            _ => ("f8_e4m3", columns),
        };
        let mut tensors = vec![ParameterBinding::new(&self.weight, dtype, &[rows, width])];
        if let Some(scale) = &self.scale {
            let scale_rows = if self.layout == MatrixLayout::Fp8Block32 {
                rows.div_ceil(32)
            } else {
                rows
            };
            tensors.push(ParameterBinding::new(
                scale,
                "f8_e8m0",
                &[scale_rows, columns.div_ceil(32)],
            ));
        }
        tensors
    }
}

#[derive(Debug, Clone)]
pub struct ParameterBinding {
    name: String,
    dtype: &'static str,
    shape: Vec<usize>,
}

impl ParameterBinding {
    fn new(name: &str, dtype: &'static str, shape: &[usize]) -> Self {
        Self {
            name: name.into(),
            dtype,
            shape: shape.into(),
        }
    }
    pub fn name(&self) -> &str {
        &self.name
    }
    pub fn dtype(&self) -> &'static str {
        self.dtype
    }
    pub fn shape(&self) -> &[usize] {
        &self.shape
    }
    pub fn bytes(&self) -> u64 {
        // Only the fixed, bounded official shapes construct these bindings.
        self.shape.iter().map(|&n| n as u64).product::<u64>()
            * match self.dtype {
                "bf16" => 2,
                "f32" => 4,
                _ => 1,
            }
    }

    fn validate(&self, checkpoint: &CheckpointWeights) -> Result<()> {
        let storage = checkpoint.storage(&self.name)?;
        if storage.dtype != self.dtype
            || storage.shape != self.shape
            || storage.bytes != self.bytes()
        {
            return Err(invalid(&format!(
                "official tensor {} differs from its declared dtype/shape/bytes",
                self.name
            )));
        }
        Ok(())
    }

    /// Small BF16/F32 parameters retain their exported precision. Large or
    /// quantized matrices must use bind()/DeviceMatrix's bounded row upload.
    pub fn load(
        &self,
        checkpoint: &CheckpointWeights,
        device: &CudaLayerDevice,
        cancellation: &CancellationToken,
    ) -> Result<Tensor> {
        check_cancel(cancellation)?;
        let dtype = match self.dtype {
            "bf16" => DType::BF16,
            "f32" => DType::F32,
            _ => {
                return Err(invalid(
                    "raw quantized parameters require the explicit matrix path",
                ))
            }
        };
        self.validate(checkpoint)?;
        device
            .limits()
            .checked_elements(&self.shape, "official parameter")?;
        if self.bytes() > device.limits().max_resident_weight_bytes {
            return Err(invalid(
                "official parameter exceeds its resident byte bound",
            ));
        }
        // Includes raw upload, validation intermediates and immutable output;
        // a model-wide Power reservation remains required before activation.
        let live = self
            .bytes()
            .checked_mul(16)
            .ok_or_else(|| invalid("official parameter live byte estimate overflowed"))?;
        device
            .limits()
            .checked_state_bytes(live, "official parameter upload")?;
        let raw = checkpoint.read_range(&self.name, 0, self.bytes(), cancellation)?;
        let tensor =
            Tensor::from_raw_buffer(raw.bytes(), dtype, &self.shape, device.tensor_device())
                .map_err(tensor_error)?
                .detach();
        check_cancel(cancellation)?;
        validate_finite(&tensor, false, "official parameter")?;
        check_cancel(cancellation)?;
        Ok(tensor)
    }
}

pub struct MhcBindings {
    pub projection: ParameterBinding,
    pub scale: ParameterBinding,
    pub base: ParameterBinding,
}

pub struct AttentionBindings {
    pub query_a: MatrixBinding,
    pub query_b: MatrixBinding,
    pub window_kv: MatrixBinding,
    pub output_a: MatrixBinding,
    pub output_b: MatrixBinding,
    pub query_norm: ParameterBinding,
    pub kv_norm: ParameterBinding,
    pub norm: ParameterBinding,
    pub sink: ParameterBinding,
    pub mhc: MhcBindings,
}

pub struct ExpertBindings {
    pub gate: MatrixBinding,
    pub up: MatrixBinding,
    pub down: MatrixBinding,
}
pub struct FfnBindings {
    pub router: MatrixBinding,
    pub router_bias: ParameterBinding,
    pub image_router_bias: ParameterBinding,
    pub shared: ExpertBindings,
    pub routed: Vec<ExpertBindings>,
    pub norm: ParameterBinding,
    pub mhc: MhcBindings,
}
pub struct CompressorBindings {
    pub value: MatrixBinding,
    pub gate: Option<MatrixBinding>,
    pub norm: ParameterBinding,
}
pub struct IndexQueryBindings {
    pub query: MatrixBinding,
    pub head_weights: MatrixBinding,
}
pub struct IndexKeyBindings {
    pub key: MatrixBinding,
    pub norm: ParameterBinding,
}
pub struct EngramBindings {
    pub table: MatrixBinding,
    pub projection: MatrixBinding,
    pub query: ParameterBinding,
    pub key: ParameterBinding,
}

pub struct LayerWeightSchema {
    layer: usize,
    pub attention: AttentionBindings,
    pub ffn: FfnBindings,
    pub compressor: Option<CompressorBindings>,
    pub index_query: Option<IndexQueryBindings>,
    pub index_key: Option<IndexKeyBindings>,
    pub engram: Option<EngramBindings>,
}

impl LayerWeightSchema {
    pub fn new(binding: &LayerBinding) -> Self {
        let layer = binding.layer();
        let root = format!("layers.{layer}");
        let matrix = |name: &str, layout, shape| {
            MatrixBinding::new(&format!("{root}.{name}"), layout, shape)
        };
        let parameter = |name: &str, dtype, shape: &[usize]| {
            ParameterBinding::new(&format!("{root}.{name}"), dtype, shape)
        };
        let mhc = |kind: &str| MhcBindings {
            projection: parameter(&format!("hc_{kind}_fn"), "f32", &[24, 20480]),
            scale: parameter(&format!("hc_{kind}_scale"), "f32", &[3]),
            base: parameter(&format!("hc_{kind}_base"), "f32", &[24]),
        };
        let expert = |prefix: &str, layout| ExpertBindings {
            gate: matrix(&format!("{prefix}.w1"), layout, [2304, 5120]),
            up: matrix(&format!("{prefix}.w3"), layout, [2304, 5120]),
            down: matrix(&format!("{prefix}.w2"), layout, [5120, 2304]),
        };
        let attention = AttentionBindings {
            query_a: matrix("attn.wq_a", MatrixLayout::Fp8Block32, [1280, 5120]),
            query_b: matrix("attn.wq_b", MatrixLayout::Fp8Block32, [32768, 1280]),
            window_kv: matrix("attn.wkv", MatrixLayout::Fp8Block32, [512, 5120]),
            output_a: matrix("attn.wo_a", MatrixLayout::Fp8Block32, [8192, 4096]),
            output_b: matrix("attn.wo_b", MatrixLayout::Fp8Block32, [5120, 8192]),
            query_norm: parameter("attn.q_norm.weight", "bf16", &[1280]),
            kv_norm: parameter("attn.kv_norm.weight", "bf16", &[512]),
            norm: parameter("attn_norm.weight", "bf16", &[5120]),
            sink: parameter("attn.attn_sink", "f32", &[64]),
            mhc: mhc("attn"),
        };
        let ffn = FfnBindings {
            router: matrix("ffn.gate", MatrixLayout::Bf16, [384, 5120]),
            router_bias: parameter("ffn.gate.bias", "f32", &[384]),
            image_router_bias: parameter("ffn.gate.bias_vl", "f32", &[384]),
            shared: expert("ffn.shared_experts", MatrixLayout::Fp8Block32),
            routed: (0..384)
                .map(|expert_id| {
                    expert(
                        &format!("ffn.experts.{expert_id}"),
                        MatrixLayout::Mxfp4Block32,
                    )
                })
                .collect(),
            norm: parameter("ffn_norm.weight", "bf16", &[5120]),
            mhc: mhc("ffn"),
        };
        Self {
            layer,
            attention,
            ffn,
            compressor: binding.is_kv_source().then(|| CompressorBindings {
                value: matrix("attn.compressor.wkv", MatrixLayout::Bf16, [512, 5120]),
                gate: (binding.compression() == Some(CompressionKind::C2))
                    .then(|| matrix("attn.compressor.wgate", MatrixLayout::Bf16, [512, 5120])),
                norm: parameter("attn.compressor.norm.weight", "bf16", &[512]),
            }),
            index_query: binding.is_index_topk_source().then(|| IndexQueryBindings {
                query: matrix("attn.indexer.wq_b", MatrixLayout::Fp8Block32, [4096, 1280]),
                head_weights: matrix("attn.indexer.weights_proj", MatrixLayout::Bf16, [32, 5120]),
            }),
            index_key: binding.is_index_k_source().then(|| IndexKeyBindings {
                key: matrix("attn.indexer.wk", MatrixLayout::Bf16, [128, 512]),
                norm: parameter("attn.indexer.k_norm.weight", "bf16", &[128]),
            }),
            engram: binding.engram().map(|e| EngramBindings {
                table: matrix("engram.embed", MatrixLayout::Fp8Row32, [e.rows(), 256]),
                projection: matrix("engram.wkv", MatrixLayout::Fp8Block32, [25600, 6144]),
                query: parameter("engram.q_weight", "bf16", &[4, 5120]),
                key: parameter("engram.k_weight", "bf16", &[4, 5120]),
            }),
        }
    }

    pub fn layer(&self) -> usize {
        self.layer
    }

    /// Exact storage inventory for a base layer, including scales and parameters.
    /// Calling this does not read any tensor payload.
    pub fn tensors(&self) -> Vec<ParameterBinding> {
        let a = &self.attention;
        let f = &self.ffn;
        let mut matrices = vec![
            &a.query_a,
            &a.query_b,
            &a.window_kv,
            &a.output_a,
            &a.output_b,
            &f.router,
        ];
        for expert in std::iter::once(&f.shared).chain(&f.routed) {
            matrices.extend([&expert.gate, &expert.up, &expert.down]);
        }
        let mut parameters = vec![
            &a.query_norm,
            &a.kv_norm,
            &a.norm,
            &a.sink,
            &f.norm,
            &f.router_bias,
            &f.image_router_bias,
        ];
        for mhc in [&a.mhc, &f.mhc] {
            parameters.extend([&mhc.projection, &mhc.scale, &mhc.base]);
        }
        if let Some(c) = &self.compressor {
            matrices.push(&c.value);
            matrices.extend(c.gate.iter());
            parameters.push(&c.norm);
        }
        if let Some(i) = &self.index_query {
            matrices.extend([&i.query, &i.head_weights]);
        }
        if let Some(i) = &self.index_key {
            matrices.push(&i.key);
            parameters.push(&i.norm);
        }
        if let Some(e) = &self.engram {
            matrices.extend([&e.table, &e.projection]);
            parameters.extend([&e.query, &e.key]);
        }
        let mut tensors = matrices
            .into_iter()
            .flat_map(MatrixBinding::tensors)
            .collect::<Vec<_>>();
        tensors.extend(parameters.into_iter().cloned());
        tensors.sort_by(|a, b| a.name.cmp(&b.name));
        tensors
    }

    pub fn validate(
        &self,
        checkpoint: &CheckpointWeights,
        cancellation: &CancellationToken,
    ) -> Result<()> {
        validate_namespace(
            checkpoint,
            &format!("layers.{}.", self.layer),
            &self.tensors(),
            cancellation,
        )
    }
}

pub struct TextWeightSchema {
    pub embedding: MatrixBinding,
    pub head: MatrixBinding,
    pub norm: ParameterBinding,
    layers: Vec<LayerWeightSchema>,
}
impl TextWeightSchema {
    pub fn layers(&self) -> &[LayerWeightSchema] {
        &self.layers
    }

    pub fn new(architecture: &TextArchitecture) -> Self {
        Self {
            embedding: MatrixBinding::new("embed", MatrixLayout::Bf16, [129280, 5120]),
            head: MatrixBinding::new("head", MatrixLayout::Bf16, [129280, 5120]),
            norm: ParameterBinding::new("norm.weight", "bf16", &[5120]),
            layers: architecture
                .layers()
                .iter()
                .map(LayerWeightSchema::new)
                .collect(),
        }
    }

    pub fn validate(
        &self,
        checkpoint: &CheckpointWeights,
        cancellation: &CancellationToken,
    ) -> Result<()> {
        check_cancel(cancellation)?;
        self.embedding.bind(checkpoint)?;
        self.head.bind(checkpoint)?;
        self.norm.validate(checkpoint)?;
        for layer in &self.layers {
            layer.validate(checkpoint, cancellation)?;
        }
        Ok(())
    }
}

fn validate_namespace(
    checkpoint: &CheckpointWeights,
    prefix: &str,
    tensors: &[ParameterBinding],
    cancellation: &CancellationToken,
) -> Result<()> {
    let names = tensors
        .iter()
        .map(|t| t.name.as_str())
        .collect::<BTreeSet<_>>();
    let actual = checkpoint
        .store
        .inventory()
        .filter_map(|t| t.name.starts_with(prefix).then_some(t.name.as_str()))
        .collect::<BTreeSet<_>>();
    if names.len() != tensors.len() || names != actual {
        return Err(invalid(&format!("official tensor namespace {prefix} contains missing, unexpected or duplicate parameters")));
    }
    for tensor in tensors {
        check_cancel(cancellation)?;
        tensor.validate(checkpoint)?;
    }
    Ok(())
}

fn check_cancel(cancellation: &CancellationToken) -> Result<()> {
    if cancellation.is_cancelled() {
        return Err(invalid("official parameter binding was cancelled"));
    }
    Ok(())
}

#[cfg(test)]
mod tests;
