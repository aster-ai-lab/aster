//! V4.1 index projections, preserving each BF16/FP8/FP4 boundary.
//!
//! Q consumes the main query's normalized low-rank latent; K consumes the
//! main compressed KV after its norm and before its RoPE. Only K has its own
//! learned RMSNorm. The runtime passes no HadamardArgs and applies no extra
//! head-count/softmax scaling to the effective BF16 head weights.
//! Apache-2.0 semantics: DeepInfer index_attn.py, attn.py and compress.py at
//! 9bd9cf243fc53ef9afbf8f04d391d989117d6ab9. Unfused control, not a GPU proof.

use aster_power::error::{PowerError, Result};
use candle_core::{DType, Tensor};
use tokio_util::sync::CancellationToken;

use crate::{
    layers::{
        invalid,
        linear::{activation::Fp8Activations, DeviceMatrix},
        norm::EffectiveRmsNorm,
        quant::{Fp4IndexRows, INDEX_HEAD_DIM},
        rope::{RopeDirection, RopeLayout, RotaryTransform},
        tensor_error, CudaLayerDevice,
    },
    weights::MatrixLayout,
};

pub struct ProjectedIndexQuery {
    rows: Fp4IndexRows,
    head_weights: Tensor,
}

impl ProjectedIndexQuery {
    /// Token-major, head-minor [tokens * heads, 128] packed logical rows.
    pub fn rows(&self) -> &Fp4IndexRows {
        &self.rows
    }
    pub fn head_weights(&self) -> &Tensor {
        &self.head_weights
    }
}

pub struct IndexQueryProjection {
    device: CudaLayerDevice,
    query: DeviceMatrix,
    head_weights: DeviceMatrix,
    rope: RotaryTransform,
    heads: usize,
    resident_bytes: u64,
}

impl IndexQueryProjection {
    pub fn new(
        device: CudaLayerDevice,
        query: DeviceMatrix,
        head_weights: DeviceMatrix,
        cos_sin: Tensor,
    ) -> Result<Self> {
        let [output, rank] = query.logical_shape();
        let heads = output / INDEX_HEAD_DIM;
        if query.layout() != MatrixLayout::Fp8Block32
            || !output.is_multiple_of(INDEX_HEAD_DIM)
            || !(1..=32).contains(&heads)
            || !rank.is_multiple_of(32)
            || head_weights.layout() != MatrixLayout::Bf16
            || head_weights.logical_shape()[0] != heads
        {
            return Err(invalid("index Q requires full FP8 group-32 projection to 1..=32 heads of width 128 and a matching BF16 head-weight projection"));
        }
        validate_matrix(&device, &query)?;
        validate_matrix(&device, &head_weights)?;
        let rope = index_rope(&device, cos_sin)?;
        let resident_bytes = checked_resident(
            &device,
            &[
                query.retained_weight_bytes(),
                head_weights.retained_weight_bytes(),
                rope.retained_cache_bytes(),
            ],
        )?;
        Ok(Self {
            device,
            query,
            head_weights,
            rope,
            heads,
            resident_bytes,
        })
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
    ) -> Result<ProjectedIndexQuery> {
        check_cancel(cancellation)?;
        let tokens = check_input(
            &self.device,
            &self.rope,
            normalized_query_latent,
            self.query.logical_shape()[1],
            positions,
            false,
        )?;
        check_input(
            &self.device,
            &self.rope,
            current_hidden,
            self.head_weights.logical_shape()[1],
            positions,
            false,
        )?;
        check_live_bytes(
            &self.device,
            tokens,
            self.heads,
            self.query.logical_shape()[1] + self.head_weights.logical_shape()[1],
            self.rope.retained_cache_bytes(),
        )?;
        // FP8-cast the shared normalized latent, with no extra per-index Q norm.
        let latent = Fp8Activations::encode(&self.device, normalized_query_latent, cancellation)?;
        self.forward_packed(current_hidden, &latent, positions, cancellation)
    }

    /// Reuse the exact packed normalized latent produced by the main Q path.
    pub fn forward_packed(
        &self,
        current_hidden: &Tensor,
        normalized_query_latent: &Fp8Activations,
        positions: &[usize],
        cancellation: &CancellationToken,
    ) -> Result<ProjectedIndexQuery> {
        check_cancel(cancellation)?;
        let tokens = check_input(
            &self.device,
            &self.rope,
            current_hidden,
            self.head_weights.logical_shape()[1],
            positions,
            false,
        )?;
        if normalized_query_latent.data_bytes().dims() != [tokens, self.query.logical_shape()[1]]
            || !normalized_query_latent
                .data_bytes()
                .device()
                .same_device(self.device.tensor_device())
        {
            return Err(invalid(
                "shared packed index Q latent differs from its token/rank/device geometry",
            ));
        }
        check_live_bytes(
            &self.device,
            tokens,
            self.heads,
            self.query.logical_shape()[1] + self.head_weights.logical_shape()[1],
            self.rope.retained_cache_bytes(),
        )?;
        let query = self
            .query
            .forward_fp8(normalized_query_latent, DType::BF16, cancellation)?
            .reshape((tokens, self.heads, INDEX_HEAD_DIM))
            .map_err(tensor_error)?;
        let rotated = self
            .rope
            .forward(&query, positions, RopeDirection::Forward, cancellation)?
            .reshape((tokens * self.heads, INDEX_HEAD_DIM))
            .map_err(tensor_error)?;
        let rows = Fp4IndexRows::encode(&self.device, &rotated, cancellation)?;
        let head_weights =
            self.head_weights
                .forward_bf16(current_hidden, DType::BF16, cancellation)?;
        check_cancel(cancellation)?;
        Ok(ProjectedIndexQuery { rows, head_weights })
    }
}

pub struct IndexKeyProjection {
    device: CudaLayerDevice,
    key: DeviceMatrix,
    norm: EffectiveRmsNorm,
    rope: RotaryTransform,
    resident_bytes: u64,
}

impl IndexKeyProjection {
    pub fn new(
        device: CudaLayerDevice,
        key: DeviceMatrix,
        norm: EffectiveRmsNorm,
        cos_sin: Tensor,
    ) -> Result<Self> {
        if key.layout() != MatrixLayout::Bf16
            || key.logical_shape()[0] != INDEX_HEAD_DIM
            || norm.width() != INDEX_HEAD_DIM
            || !norm.has_effective_weight()
            || !norm
                .device()
                .tensor_device()
                .same_device(device.tensor_device())
            || norm.device().limits() != device.limits()
        {
            return Err(invalid("index K requires a full BF16 projection to width 128 and same-device learned effective RMSNorm"));
        }
        validate_matrix(&device, &key)?;
        let rope = index_rope(&device, cos_sin)?;
        let resident_bytes = checked_resident(
            &device,
            &[
                key.retained_weight_bytes(),
                norm.retained_parameter_bytes(),
                rope.retained_cache_bytes(),
            ],
        )?;
        Ok(Self {
            device,
            key,
            norm,
            rope,
            resident_bytes,
        })
    }

    pub fn retained_bytes(&self) -> u64 {
        self.resident_bytes
    }

    /// Compressed rows are already normalized by the main compressor, and
    /// have NOT had main-KV RoPE applied. Positions are the compressor's
    /// explicit positions_c, rather than a new inferred row-number sequence.
    pub fn forward(
        &self,
        pre_rope_main_kv: &Tensor,
        compressed_positions: &[usize],
        cancellation: &CancellationToken,
    ) -> Result<Fp4IndexRows> {
        check_cancel(cancellation)?;
        let rows = check_input(
            &self.device,
            &self.rope,
            pre_rope_main_kv,
            self.key.logical_shape()[1],
            compressed_positions,
            true,
        )?;
        check_live_bytes(
            &self.device,
            rows,
            1,
            self.key.logical_shape()[1],
            self.rope.retained_cache_bytes() + self.norm.retained_parameter_bytes(),
        )?;
        // C2 may have no completed pair in a chunk. Do not fabricate a key.
        if rows == 0 {
            let empty = Tensor::zeros(
                (0, INDEX_HEAD_DIM),
                DType::BF16,
                self.device.tensor_device(),
            )
            .map_err(tensor_error)?;
            return Fp4IndexRows::encode(&self.device, &empty, cancellation);
        }
        let projected = self
            .key
            .forward_bf16(pre_rope_main_kv, DType::BF16, cancellation)?;
        let normalized = self
            .norm
            .forward(&projected)?
            .unsqueeze(1)
            .map_err(tensor_error)?;
        let rotated = self
            .rope
            .forward(
                &normalized,
                compressed_positions,
                RopeDirection::Forward,
                cancellation,
            )?
            .squeeze(1)
            .map_err(tensor_error)?;
        let packed = Fp4IndexRows::encode(&self.device, &rotated, cancellation)?;
        check_cancel(cancellation)?;
        Ok(packed)
    }
}

fn index_rope(device: &CudaLayerDevice, cache: Tensor) -> Result<RotaryTransform> {
    if cache.dims().get(1) != Some(&64) {
        return Err(invalid(
            "index RoPE requires an explicit FP32 cache of rotary width 64",
        ));
    }
    RotaryTransform::from_cache(
        device.clone(),
        INDEX_HEAD_DIM,
        64,
        RopeLayout::Interleaved,
        cache,
    )
}

fn validate_matrix(device: &CudaLayerDevice, matrix: &DeviceMatrix) -> Result<()> {
    if !matrix
        .device()
        .tensor_device()
        .same_device(device.tensor_device())
        || matrix.device().limits() != device.limits()
        || matrix.source_first_row() != 0
        || matrix.source_total_rows() != matrix.logical_shape()[0]
    {
        return Err(invalid(
            "index projection requires a complete matrix on its owning device/limits",
        ));
    }
    Ok(())
}

fn checked_resident(device: &CudaLayerDevice, bytes: &[u64]) -> Result<u64> {
    let total = bytes
        .iter()
        .try_fold(0_u64, |n, b| n.checked_add(*b))
        .ok_or_else(|| invalid("index projection resident bytes overflowed"))?;
    if total > device.limits().max_resident_weight_bytes {
        return Err(invalid("index projection exceeds aggregate resident bytes"));
    }
    Ok(total)
}

fn check_input(
    device: &CudaLayerDevice,
    rope: &RotaryTransform,
    input: &Tensor,
    width: usize,
    positions: &[usize],
    allow_empty: bool,
) -> Result<usize> {
    let (rows, columns) = input.dims2().map_err(tensor_error)?;
    if columns != width
        || input.dtype() != DType::BF16
        || !input.device().same_device(device.tensor_device())
        || (!allow_empty && rows == 0)
        || rows > device.limits().max_context_tokens
        || rows != positions.len()
        || positions.iter().any(|p| *p >= rope.context_tokens())
    {
        return Err(invalid("index projection requires same-device BF16 rows, matching widths and in-cache explicit positions"));
    }
    if rows > 0 {
        device
            .limits()
            .checked_elements(input.dims(), "index projection input")?;
    }
    Ok(rows)
}

fn check_live_bytes(
    device: &CudaLayerDevice,
    rows: usize,
    heads: usize,
    input_width: usize,
    retained: u64,
) -> Result<()> {
    // Composite bound includes projection tiles, FP8 cast/decode, BF16
    // norm/RoPE outputs and FP4 cast temporaries; not a measured GPU peak.
    let input = (rows as u64)
        .checked_mul(input_width as u64)
        .and_then(|n| n.checked_mul(160));
    let output = (rows as u64)
        .checked_mul(heads as u64)
        .and_then(|n| n.checked_mul((INDEX_HEAD_DIM * 200) as u64));
    let tile = (input_width.max(INDEX_HEAD_DIM) as u64).checked_mul(32 * 64);
    let bytes = input
        .zip(output)
        .and_then(|(a, b)| a.checked_add(b))
        .zip(tile)
        .and_then(|(a, b)| a.checked_add(b))
        .and_then(|n| n.checked_add(retained))
        .and_then(|n| n.checked_add(4096))
        .ok_or_else(|| invalid("index projection composite live bytes overflowed"))?;
    device
        .limits()
        .checked_state_bytes(bytes, "index projection composite live tensors")?;
    Ok(())
}

fn check_cancel(cancellation: &CancellationToken) -> Result<()> {
    if cancellation.is_cancelled() {
        Err(PowerError::InferenceCancelled)
    } else {
        Ok(())
    }
}

#[cfg(test)]
mod tests;
