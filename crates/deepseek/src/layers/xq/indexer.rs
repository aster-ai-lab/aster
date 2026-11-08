//! Original F16 index projections with F32 activations/head weights, explicit
//! 1/sqrt(index_dim*heads), and the XQ FP4/pow2 grid. Index K consumes the
//! normalized main latent before its rotary/quantized representation.
use aster_power::error::Result;
use candle_core::{DType, Tensor};
use tokio_util::sync::CancellationToken;

use super::attention::{admit, live, positions_valid, rotary};
use super::compressor::{source_kind, validate_f16, XqPreparedCompressedKv};
use super::quant::{XqActivationFormat, XqQuantizedActivation};
use super::{check_cancel, invalid, same_device, XqRmsNorm};
use crate::{
    layers::{
        attention::projection::{add, resident, validate_owner},
        linear::xq::XqDeviceMatrix,
        quant::Fp4IndexRows,
        rope::{RopeDirection, RotaryTransform},
        tensor_error, CudaLayerDevice,
    },
    weights::xq::{storage::XqWeights, XqTensorType},
};

pub struct XqProjectedIndexQuery {
    rows: Fp4IndexRows,
    head_weights: Tensor,
}
impl XqProjectedIndexQuery {
    pub fn rows(&self) -> &Fp4IndexRows {
        &self.rows
    }
    /// Already scaled F32 weights, ready for select_xq_f32_packed.
    pub fn head_weights(&self) -> &Tensor {
        &self.head_weights
    }
}
pub struct XqIndexQueryProjection {
    device: CudaLayerDevice,
    layer: usize,
    query: XqDeviceMatrix,
    head_weights: XqDeviceMatrix,
    rope: RotaryTransform,
    heads: usize,
    resident_bytes: u64,
}
impl XqIndexQueryProjection {
    pub fn load(
        device: &CudaLayerDevice,
        weights: &XqWeights,
        layer: usize,
        cos_sin: Tensor,
        cancellation: &CancellationToken,
    ) -> Result<Self> {
        check_cancel(cancellation)?;
        query_source(layer)?;
        let prefix = format!("blk.{layer}");
        let expected = [
            (
                "indexer.attn_q_b.weight",
                XqTensorType::F16,
                vec![1280, 4096],
            ),
            ("indexer.proj.weight", XqTensorType::F16, vec![5120, 32]),
        ];
        let admitted = admit(device, weights, &prefix, &expected, &cos_sin)?;
        let query = XqDeviceMatrix::load(
            device,
            weights,
            &format!("{prefix}.indexer.attn_q_b.weight"),
            0,
            4096,
            cancellation,
        )?;
        let head_weights = XqDeviceMatrix::load(
            device,
            weights,
            &format!("{prefix}.indexer.proj.weight"),
            0,
            32,
            cancellation,
        )?;
        let output = Self::from_parts(device.clone(), layer, query, head_weights, cos_sin)?;
        if output.retained_bytes() != admitted {
            return Err(invalid(
                "XQ index-Q retained bytes differ from pre-upload admission",
            ));
        }
        check_cancel(cancellation)?;
        Ok(output)
    }
    pub(super) fn from_parts(
        device: CudaLayerDevice,
        layer: usize,
        query: XqDeviceMatrix,
        head_weights: XqDeviceMatrix,
        cos_sin: Tensor,
    ) -> Result<Self> {
        query_source(layer)?;
        let [output, rank] = query.logical_shape();
        let [heads, hidden] = head_weights.logical_shape();
        if !(1..=32).contains(&heads)
            || output != heads * 128
            || rank == 0
            || rank > 1280
            || hidden == 0
            || hidden > 5120
        {
            return Err(invalid(
                "XQ index Q requires complete F16 rank/hidden projections to 1..32 width-128 heads",
            ));
        }
        for matrix in [&query, &head_weights] {
            validate_f16(&device, matrix)?;
        }
        let rope = rotary(&device, &cos_sin, 128)?;
        let bytes = add(
            add(
                query.retained_weight_bytes(),
                head_weights.retained_weight_bytes(),
            )?,
            rope.retained_cache_bytes(),
        )?;
        let resident_bytes = resident(&device, bytes)?;
        Ok(Self {
            device,
            layer,
            query,
            head_weights,
            rope,
            heads,
            resident_bytes,
        })
    }
    pub fn layer(&self) -> usize {
        self.layer
    }
    pub fn heads(&self) -> usize {
        self.heads
    }
    pub fn retained_bytes(&self) -> u64 {
        self.resident_bytes
    }
    pub fn forward(
        &self,
        current_hidden: &Tensor,
        normalized_query_latent: &Tensor,
        positions: &[usize],
        cancellation: &CancellationToken,
    ) -> Result<XqProjectedIndexQuery> {
        check_cancel(cancellation)?;
        let tokens = validate_input(
            &self.device,
            &self.rope,
            current_hidden,
            self.head_weights.logical_shape()[1],
            positions,
            false,
        )?;
        validate_input(
            &self.device,
            &self.rope,
            normalized_query_latent,
            self.query.logical_shape()[1],
            positions,
            false,
        )?;
        self.live_tensor_bytes(tokens)?;
        let query = self
            .query
            .forward_f32_control(normalized_query_latent, cancellation)?
            .0
            .reshape((tokens, self.heads, 128))
            .map_err(tensor_error)?;
        let rotated = self
            .rope
            .forward_f32_control(&query, positions, RopeDirection::Forward, cancellation)?
            .reshape((tokens * self.heads, 128))
            .map_err(tensor_error)?;
        let rows = XqQuantizedActivation::encode(
            &self.device,
            XqActivationFormat::IndexFp4,
            &rotated,
            cancellation,
        )?
        .into_index_rows(cancellation)?;
        let projected = self
            .head_weights
            .forward_f32_control(current_hidden, cancellation)?
            .0;
        let head_weights = scale_head_weights(&projected, self.heads)?;
        check_cancel(cancellation)?;
        Ok(XqProjectedIndexQuery { rows, head_weights })
    }

    pub(crate) fn live_tensor_bytes(&self, tokens: usize) -> Result<u64> {
        let packed = self
            .query
            .retained_weight_bytes()
            .max(self.head_weights.retained_weight_bytes());
        live(
            &self.device,
            tokens,
            &[
                (
                    self.query.logical_shape()[1] + self.head_weights.logical_shape()[1],
                    64,
                ),
                (self.heads * 128, 256),
            ],
            packed,
            "XQ index Q composite live tensors",
        )
    }
}

pub struct XqIndexKeyProjection {
    device: CudaLayerDevice,
    layer: usize,
    key: XqDeviceMatrix,
    norm: XqRmsNorm,
    rope: RotaryTransform,
    resident_bytes: u64,
}
impl XqIndexKeyProjection {
    pub fn load(
        device: &CudaLayerDevice,
        weights: &XqWeights,
        layer: usize,
        cos_sin: Tensor,
        cancellation: &CancellationToken,
    ) -> Result<Self> {
        check_cancel(cancellation)?;
        source_kind(layer)?;
        let prefix = format!("blk.{layer}");
        let expected = [
            ("indexer.attn_k.weight", XqTensorType::F16, vec![512, 128]),
            ("indexer.k_norm.weight", XqTensorType::F32, vec![128]),
        ];
        let admitted = admit(device, weights, &prefix, &expected, &cos_sin)?;
        let key = XqDeviceMatrix::load(
            device,
            weights,
            &format!("{prefix}.indexer.attn_k.weight"),
            0,
            128,
            cancellation,
        )?;
        let norm = XqRmsNorm::load(
            device,
            weights,
            &format!("{prefix}.indexer.k_norm.weight"),
            128,
            cancellation,
        )?;
        let output = Self::from_parts(device.clone(), layer, key, norm, cos_sin)?;
        if output.retained_bytes() != admitted {
            return Err(invalid(
                "XQ index K retained bytes differ from pre-upload admission",
            ));
        }
        check_cancel(cancellation)?;
        Ok(output)
    }
    pub(super) fn from_parts(
        device: CudaLayerDevice,
        layer: usize,
        key: XqDeviceMatrix,
        norm: XqRmsNorm,
        cos_sin: Tensor,
    ) -> Result<Self> {
        source_kind(layer)?;
        if key.logical_shape() != [128, 512] || norm.width() != 128 {
            return Err(invalid(
                "XQ index K requires F16 [128,512] and learned F32 width-128 norm",
            ));
        }
        validate_f16(&device, &key)?;
        validate_owner(&device, norm.device())?;
        let rope = rotary(&device, &cos_sin, 128)?;
        let resident_bytes = resident(
            &device,
            add(
                add(key.retained_weight_bytes(), norm.retained_bytes())?,
                rope.retained_cache_bytes(),
            )?,
        )?;
        Ok(Self {
            device,
            layer,
            key,
            norm,
            rope,
            resident_bytes,
        })
    }
    pub fn layer(&self) -> usize {
        self.layer
    }
    pub fn retained_bytes(&self) -> u64 {
        self.resident_bytes
    }
    pub fn forward_compressed(
        &self,
        compression: &XqPreparedCompressedKv,
        cancellation: &CancellationToken,
    ) -> Result<Fp4IndexRows> {
        if compression.layer() != self.layer {
            return Err(invalid("XQ index K/compressor source layers differ"));
        }
        self.forward(
            compression.pre_rope_kv(),
            compression.rotary_positions(),
            cancellation,
        )
    }
    pub fn forward(
        &self,
        pre_rope_main_kv: &Tensor,
        compressed_rotary_positions: &[usize],
        cancellation: &CancellationToken,
    ) -> Result<Fp4IndexRows> {
        check_cancel(cancellation)?;
        let tokens = validate_input(
            &self.device,
            &self.rope,
            pre_rope_main_kv,
            512,
            compressed_rotary_positions,
            true,
        )?;
        if tokens == 0 {
            let empty = Tensor::zeros((0, 128), DType::F32, self.device.tensor_device())
                .map_err(tensor_error)?;
            return XqQuantizedActivation::encode(
                &self.device,
                XqActivationFormat::IndexFp4,
                &empty,
                cancellation,
            )?
            .into_index_rows(cancellation);
        }
        self.live_tensor_bytes(tokens)?;
        let projected = self
            .key
            .forward_f32_control(pre_rope_main_kv, cancellation)?
            .0;
        let normalized = self
            .norm
            .forward(&projected, cancellation)?
            .unsqueeze(1)
            .map_err(tensor_error)?;
        let rotated = self
            .rope
            .forward_f32_control(
                &normalized,
                compressed_rotary_positions,
                RopeDirection::Forward,
                cancellation,
            )?
            .squeeze(1)
            .map_err(tensor_error)?;
        let rows = XqQuantizedActivation::encode(
            &self.device,
            XqActivationFormat::IndexFp4,
            &rotated,
            cancellation,
        )?
        .into_index_rows(cancellation)?;
        check_cancel(cancellation)?;
        Ok(rows)
    }

    pub(crate) fn live_tensor_bytes(&self, tokens: usize) -> Result<u64> {
        live(
            &self.device,
            tokens,
            &[(512, 64), (128, 256)],
            self.key.retained_weight_bytes(),
            "XQ index K composite live tensors",
        )
    }
}

fn query_source(layer: usize) -> Result<()> {
    match layer {
        2 | 8 | 14 | 20 | 24 | 28 | 32 | 36 => Ok(()),
        _ => Err(invalid(
            "XQ index-Q sources are exactly layers 2,8,14,20,24,28,32,36",
        )),
    }
}
fn validate_input(
    device: &CudaLayerDevice,
    rope: &RotaryTransform,
    input: &Tensor,
    width: usize,
    positions: &[usize],
    allow_empty: bool,
) -> Result<usize> {
    same_device(device, input)?;
    let (rows, columns) = input.dims2().map_err(tensor_error)?;
    if input.dtype() != DType::F32 || columns != width || positions.len() != rows {
        return Err(invalid(
            "XQ index projection requires matching F32 rows and explicit positions",
        ));
    }
    if rows == 0 && allow_empty {
        return Ok(0);
    }
    positions_valid(device, rope, rows, positions)?;
    device
        .limits()
        .checked_elements(input.dims(), "XQ index projection input")?;
    Ok(rows)
}
fn scale_head_weights(projected: &Tensor, heads: usize) -> Result<Tensor> {
    if projected.dtype() != DType::F32
        || !(1..=32).contains(&heads)
        || projected.dims2().map_err(tensor_error)?.1 != heads
    {
        return Err(invalid(
            "XQ head-weight scaling requires matching F32 index heads",
        ));
    }
    let scale = (128_f32 * heads as f32).sqrt().recip();
    let output = projected
        .affine(f64::from(scale), 0.)
        .map_err(tensor_error)?
        .detach();
    crate::layers::validate_finite(&output, false, "XQ scaled F32 index head weights")?;
    Ok(output)
}

#[cfg(test)]
pub(crate) mod tests;
