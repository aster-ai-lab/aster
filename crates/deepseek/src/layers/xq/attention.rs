//! Pinned XQ main-query/window and grouped-output projections. Original Q8_0
//! bytes stay resident; activations, learned norms and RoPE remain F32. The
//! window explicitly enters XQ's FP8 grid. Compression/index selection and
//! request-state composition are separate model-owned obligations.

use aster_power::error::Result;
use candle_core::{DType, Tensor};
use std::sync::Arc;
use tokio_util::sync::CancellationToken;

pub mod sublayer;

use super::quant::{XqActivationFormat, XqQuantizedActivation};
use super::{check_cancel, invalid, same_device, XqRmsNorm};
use crate::layers::{
    attention::projection::{add, resident, validate_owner},
    kv::{MainKvRows, MAIN_KV_DIM},
    linear::xq::XqDeviceMatrix,
    rope::{RopeDirection, RopeLayout, RotaryTransform},
    tensor_error, validate_finite, CudaLayerDevice,
};
use crate::weights::xq::{storage::XqWeights, XqTensorType};

pub struct XqProjectedAttentionInput {
    owner: Arc<()>,
    query: Tensor,
    normalized_query: Tensor,
    window: MainKvRows,
    positions: Vec<usize>,
}
impl XqProjectedAttentionInput {
    pub fn query(&self) -> &Tensor {
        &self.query
    }
    pub fn normalized_query(&self) -> &Tensor {
        &self.normalized_query
    }
    pub fn window_rows(&self) -> &MainKvRows {
        &self.window
    }
    pub fn positions(&self) -> &[usize] {
        &self.positions
    }
}

struct InputParts {
    query_a: XqDeviceMatrix,
    query_b: XqDeviceMatrix,
    window: XqDeviceMatrix,
    query_norm: XqRmsNorm,
    window_norm: XqRmsNorm,
    cos_sin: Tensor,
}

pub struct XqAttentionInputProjection {
    owner: Arc<()>,
    device: CudaLayerDevice,
    query_a: XqDeviceMatrix,
    query_b: XqDeviceMatrix,
    window: XqDeviceMatrix,
    query_norm: XqRmsNorm,
    window_norm: XqRmsNorm,
    rope: RotaryTransform,
    heads: usize,
    resident_bytes: u64,
}

impl XqAttentionInputProjection {
    pub fn load(
        device: &CudaLayerDevice,
        weights: &XqWeights,
        layer: usize,
        cos_sin: Tensor,
        cancellation: &CancellationToken,
    ) -> Result<Self> {
        check_cancel(cancellation)?;
        let prefix = prefix(layer)?;
        let expected = [
            ("attn_q_a.weight", XqTensorType::Q8_0, vec![5120, 1280]),
            ("attn_q_b.weight", XqTensorType::Q8_0, vec![1280, 32768]),
            ("attn_kv.weight", XqTensorType::Q8_0, vec![5120, 512]),
            ("attn_q_a_norm.weight", XqTensorType::F32, vec![1280]),
            ("attn_kv_a_norm.weight", XqTensorType::F32, vec![512]),
        ];
        let admitted = admit(device, weights, &prefix, &expected, &cos_sin)?;
        let matrix = |suffix: &str, rows| {
            XqDeviceMatrix::load(
                device,
                weights,
                &format!("{prefix}.{suffix}"),
                0,
                rows,
                cancellation,
            )
        };
        let norm = |suffix: &str, width| {
            XqRmsNorm::load(
                device,
                weights,
                &format!("{prefix}.{suffix}"),
                width,
                cancellation,
            )
        };
        let output = Self::from_parts(
            device.clone(),
            InputParts {
                query_a: matrix("attn_q_a.weight", 1280)?,
                query_b: matrix("attn_q_b.weight", 32768)?,
                window: matrix("attn_kv.weight", 512)?,
                query_norm: norm("attn_q_a_norm.weight", 1280)?,
                window_norm: norm("attn_kv_a_norm.weight", 512)?,
                cos_sin,
            },
        )?;
        if output.retained_bytes() != admitted {
            return Err(invalid(
                "XQ input retained bytes differ from pre-upload admission",
            ));
        }
        check_cancel(cancellation)?;
        Ok(output)
    }

    fn from_parts(device: CudaLayerDevice, parts: InputParts) -> Result<Self> {
        let [rank, hidden] = parts.query_a.logical_shape();
        let output_width = parts.query_b.logical_shape()[0];
        if rank == 0
            || rank > 1280
            || !rank.is_multiple_of(32)
            || hidden == 0
            || hidden > 5120
            || !hidden.is_multiple_of(32)
            || !output_width.is_multiple_of(MAIN_KV_DIM)
            || !(1..=64).contains(&(output_width / MAIN_KV_DIM))
            || parts.query_b.logical_shape()[1] != rank
            || parts.window.logical_shape() != [MAIN_KV_DIM, hidden]
            || parts.query_norm.width() != rank
            || parts.window_norm.width() != MAIN_KV_DIM
        {
            return Err(invalid("XQ input requires complete Q8 query/window geometry, learned F32 norms and 1..64 width-512 heads"));
        }
        let mut bytes = 0;
        for matrix in [&parts.query_a, &parts.query_b, &parts.window] {
            validate_matrix(&device, matrix)?;
            bytes = add(bytes, matrix.retained_weight_bytes())?;
        }
        for norm in [&parts.query_norm, &parts.window_norm] {
            validate_owner(&device, norm.device())?;
            bytes = add(bytes, norm.retained_bytes())?;
        }
        let rope = main_rope(&device, &parts.cos_sin)?;
        let resident_bytes = resident(&device, add(bytes, rope.retained_cache_bytes())?)?;
        Ok(Self {
            owner: Arc::new(()),
            device,
            heads: output_width / MAIN_KV_DIM,
            query_a: parts.query_a,
            query_b: parts.query_b,
            window: parts.window,
            query_norm: parts.query_norm,
            window_norm: parts.window_norm,
            rope,
            resident_bytes,
        })
    }
    pub fn heads(&self) -> usize {
        self.heads
    }
    pub fn hidden_size(&self) -> usize {
        self.query_a.logical_shape()[1]
    }
    pub fn retained_bytes(&self) -> u64 {
        self.resident_bytes
    }
    pub fn forward(
        &self,
        current_hidden: &Tensor,
        positions: &[usize],
        cancellation: &CancellationToken,
    ) -> Result<XqProjectedAttentionInput> {
        check_cancel(cancellation)?;
        same_device(&self.device, current_hidden)?;
        let (tokens, hidden) = current_hidden.dims2().map_err(tensor_error)?;
        if current_hidden.dtype() != DType::F32 || hidden != self.hidden_size() {
            return Err(invalid(
                "XQ input projection requires matching F32 hidden rows",
            ));
        }
        positions_valid(&self.device, &self.rope, tokens, positions)?;
        let rank = self.query_a.logical_shape()[0];
        let packed = [&self.query_a, &self.query_b, &self.window]
            .iter()
            .map(|m| m.retained_weight_bytes())
            .max()
            .unwrap_or(0);
        live(
            &self.device,
            tokens,
            &[
                (hidden, 64),
                (rank, 128),
                (self.heads * MAIN_KV_DIM, 128),
                (MAIN_KV_DIM, 256),
            ],
            packed,
            "XQ input projections composite live tensors",
        )?;
        let query_a = self
            .query_a
            .forward_f32_control(current_hidden, cancellation)?
            .0;
        let normalized_query = self.query_norm.forward(&query_a, cancellation)?;
        let query = self
            .query_b
            .forward_f32_control(&normalized_query, cancellation)?
            .0
            .reshape((tokens, self.heads, MAIN_KV_DIM))
            .map_err(tensor_error)?;
        let query = self.rope.forward_f32_control(
            &query,
            positions,
            RopeDirection::Forward,
            cancellation,
        )?;
        let window = self
            .window
            .forward_f32_control(current_hidden, cancellation)?
            .0;
        let window = self
            .window_norm
            .forward(&window, cancellation)?
            .unsqueeze(1)
            .map_err(tensor_error)?;
        let window = self
            .rope
            .forward_f32_control(&window, positions, RopeDirection::Forward, cancellation)?
            .squeeze(1)
            .map_err(tensor_error)?;
        let window = XqQuantizedActivation::encode(
            &self.device,
            XqActivationFormat::WindowFp8,
            &window,
            cancellation,
        )?
        .into_main_rows(cancellation)?;
        check_cancel(cancellation)?;
        Ok(XqProjectedAttentionInput {
            owner: Arc::clone(&self.owner),
            query,
            normalized_query,
            window,
            positions: positions.to_vec(),
        })
    }
}

struct OutputParts {
    group_a: XqDeviceMatrix,
    output_b: XqDeviceMatrix,
    cos_sin: Tensor,
    heads: usize,
    groups: usize,
    rank: usize,
}

pub struct XqAttentionOutputProjection {
    device: CudaLayerDevice,
    group_a: XqDeviceMatrix,
    output_b: XqDeviceMatrix,
    heads: usize,
    groups: usize,
    rank: usize,
    rope: RotaryTransform,
    resident_bytes: u64,
}
impl XqAttentionOutputProjection {
    pub fn load(
        device: &CudaLayerDevice,
        weights: &XqWeights,
        layer: usize,
        cos_sin: Tensor,
        cancellation: &CancellationToken,
    ) -> Result<Self> {
        check_cancel(cancellation)?;
        let prefix = prefix(layer)?;
        let expected = [
            ("attn_output_a.weight", XqTensorType::Q8_0, vec![4096, 8192]),
            ("attn_output_b.weight", XqTensorType::Q8_0, vec![8192, 5120]),
        ];
        let admitted = admit(device, weights, &prefix, &expected, &cos_sin)?;
        let load = |suffix: &str, rows| {
            XqDeviceMatrix::load(
                device,
                weights,
                &format!("{prefix}.{suffix}"),
                0,
                rows,
                cancellation,
            )
        };
        let output = Self::from_parts(
            device.clone(),
            OutputParts {
                group_a: load("attn_output_a.weight", 8192)?,
                output_b: load("attn_output_b.weight", 5120)?,
                cos_sin,
                heads: 64,
                groups: 8,
                rank: 1024,
            },
        )?;
        if output.retained_bytes() != admitted {
            return Err(invalid(
                "XQ output retained bytes differ from pre-upload admission",
            ));
        }
        check_cancel(cancellation)?;
        Ok(output)
    }
    fn from_parts(device: CudaLayerDevice, parts: OutputParts) -> Result<Self> {
        if !(1..=64).contains(&parts.heads)
            || !(1..=8).contains(&parts.groups)
            || parts.rank == 0
            || parts.rank > 1024
            || !parts.rank.is_multiple_of(32)
            || !(parts.heads * MAIN_KV_DIM).is_multiple_of(parts.groups)
        {
            return Err(invalid(
                "XQ output grouping requires 1..64 heads, 1..8 groups and group-32 rank 32..1024",
            ));
        }
        let group_width = parts.heads * MAIN_KV_DIM / parts.groups;
        let [hidden, latent] = parts.output_b.logical_shape();
        if hidden == 0
            || hidden > 5120
            || !hidden.is_multiple_of(32)
            || !group_width.is_multiple_of(32)
            || parts.group_a.logical_shape() != [parts.groups * parts.rank, group_width]
            || latent != parts.groups * parts.rank
        {
            return Err(invalid(
                "XQ output matrices differ from exact grouped A / concatenated B geometry",
            ));
        }
        for matrix in [&parts.group_a, &parts.output_b] {
            validate_matrix(&device, matrix)?;
        }
        let rope = main_rope(&device, &parts.cos_sin)?;
        let bytes = add(
            add(
                parts.group_a.retained_weight_bytes(),
                parts.output_b.retained_weight_bytes(),
            )?,
            rope.retained_cache_bytes(),
        )?;
        let resident_bytes = resident(&device, bytes)?;
        Ok(Self {
            device,
            group_a: parts.group_a,
            output_b: parts.output_b,
            heads: parts.heads,
            groups: parts.groups,
            rank: parts.rank,
            rope,
            resident_bytes,
        })
    }
    pub fn hidden_size(&self) -> usize {
        self.output_b.logical_shape()[0]
    }
    pub fn retained_bytes(&self) -> u64 {
        self.resident_bytes
    }
    pub fn forward(
        &self,
        attention: &Tensor,
        positions: &[usize],
        cancellation: &CancellationToken,
    ) -> Result<Tensor> {
        check_cancel(cancellation)?;
        same_device(&self.device, attention)?;
        let (tokens, heads, width) = attention.dims3().map_err(tensor_error)?;
        if attention.dtype() != DType::F32 || heads != self.heads || width != MAIN_KV_DIM {
            return Err(invalid(
                "XQ output projection requires matching F32 attention heads",
            ));
        }
        positions_valid(&self.device, &self.rope, tokens, positions)?;
        let packed = self
            .group_a
            .retained_weight_bytes()
            .max(self.output_b.retained_weight_bytes());
        live(
            &self.device,
            tokens,
            &[
                (heads * width, 128),
                (self.groups * self.rank, 96),
                (self.hidden_size(), 64),
            ],
            packed,
            "XQ grouped output composite live tensors",
        )?;
        let attention = self.rope.forward_f32_control(
            attention,
            positions,
            RopeDirection::Conjugate,
            cancellation,
        )?;
        let group_width = heads * width / self.groups;
        let grouped = attention
            .reshape((tokens, self.groups, group_width))
            .map_err(tensor_error)?;
        let mut parts = Vec::with_capacity(self.groups);
        for group in 0..self.groups {
            check_cancel(cancellation)?;
            let input = grouped
                .narrow(1, group, 1)
                .and_then(|x| x.squeeze(1))
                .map_err(tensor_error)?;
            parts.push(
                self.group_a
                    .forward_rows_f32_control(&input, group * self.rank, self.rank, cancellation)?
                    .0,
            );
        }
        let latent = Tensor::cat(&parts, 1).map_err(tensor_error)?;
        let output = self.output_b.forward_f32_control(&latent, cancellation)?.0;
        check_cancel(cancellation)?;
        Ok(output)
    }
}

fn prefix(layer: usize) -> Result<String> {
    if layer >= 40 {
        Err(invalid("XQ attention requires backbone layer 0..39"))
    } else {
        Ok(format!("blk.{layer}"))
    }
}
fn main_rope(device: &CudaLayerDevice, cache: &Tensor) -> Result<RotaryTransform> {
    rotary(device, cache, MAIN_KV_DIM)
}

pub(super) fn rotary(
    device: &CudaLayerDevice,
    cache: &Tensor,
    width: usize,
) -> Result<RotaryTransform> {
    if !matches!(width, 128 | 512) {
        return Err(invalid(
            "XQ rotary width requires 128 (index) or 512 (main) channels",
        ));
    }
    if cache.dims2().map_err(tensor_error)?.1 != 64 {
        return Err(invalid("XQ main RoPE requires exactly 64 tail channels"));
    }
    RotaryTransform::from_cache(
        device.clone(),
        width,
        width - 64,
        RopeLayout::Interleaved,
        cache.clone(),
    )
}
fn validate_matrix(device: &CudaLayerDevice, matrix: &XqDeviceMatrix) -> Result<()> {
    validate_owner(device, matrix.device())?;
    if matrix.kind() != XqTensorType::Q8_0 {
        return Err(invalid(
            "XQ main attention projection requires original Q8_0 weights",
        ));
    }
    Ok(())
}
pub(super) fn positions_valid(
    device: &CudaLayerDevice,
    rope: &RotaryTransform,
    tokens: usize,
    positions: &[usize],
) -> Result<()> {
    if tokens == 0
        || tokens > device.limits().max_context_tokens
        || positions.len() != tokens
        || positions.iter().any(|p| *p >= rope.context_tokens())
    {
        return Err(invalid(
            "XQ attention requires nonempty bounded rows and explicit in-cache positions",
        ));
    }
    let bytes = tokens
        .checked_mul(8)
        .ok_or_else(|| invalid("XQ positions bytes overflowed"))?;
    if bytes > device.limits().max_input_bytes {
        return Err(invalid("XQ positions exceed input metadata budget"));
    }
    Ok(())
}
pub(super) fn admit(
    device: &CudaLayerDevice,
    weights: &XqWeights,
    prefix: &str,
    expected: &[(&str, XqTensorType, Vec<u64>)],
    cache: &Tensor,
) -> Result<u64> {
    same_device(device, cache)?;
    let (context, width) = cache.dims2().map_err(tensor_error)?;
    if cache.dtype() != DType::F32
        || width != 64
        || context == 0
        || context > device.limits().max_context_tokens
    {
        return Err(invalid(
            "XQ attention requires a bounded F32 64-channel rotary cache",
        ));
    }
    let mut bytes = (cache.elem_count() as u64)
        .checked_mul(4)
        .ok_or_else(|| invalid("XQ rotary bytes overflowed"))?;
    for (suffix, kind, shape) in expected {
        let tensor = weights.tensor(&format!("{prefix}.{suffix}"))?;
        if tensor.kind != *kind || tensor.shape != *shape {
            return Err(invalid(
                "XQ attention parameter differs from the exact pinned exported schema",
            ));
        }
        bytes = add(bytes, tensor.bytes)?;
    }
    let admitted = resident(device, bytes)?;
    device
        .limits()
        .checked_elements(cache.dims(), "XQ admitted rotary cache")?;
    let live = (cache.elem_count() as u64)
        .checked_mul(32)
        .ok_or_else(|| invalid("XQ rotary validation bytes overflowed"))?;
    device
        .limits()
        .checked_state_bytes(live, "XQ pre-upload rotary validation")?;
    validate_finite(cache, false, "XQ admitted rotary cache")?;
    Ok(admitted)
}
pub(super) fn live(
    device: &CudaLayerDevice,
    tokens: usize,
    parts: &[(usize, u64)],
    packed: u64,
    label: &str,
) -> Result<u64> {
    let mut bytes = packed
        .checked_mul(2)
        .ok_or_else(|| invalid("XQ projection live bytes overflowed"))?;
    for (width, factor) in parts {
        let count = device.limits().checked_elements(&[tokens, *width], label)? as u64;
        bytes = add(
            bytes,
            count
                .checked_mul(*factor)
                .ok_or_else(|| invalid("XQ composite live bytes overflowed"))?,
        )?;
    }
    device.limits().checked_state_bytes(bytes, label)?;
    Ok(bytes)
}

#[cfg(test)]
mod tests;
