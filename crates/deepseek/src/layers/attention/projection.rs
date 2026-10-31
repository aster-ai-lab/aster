//! Main-query A -> learned norm -> B, with current-hidden SWA projection.
//! All three matrices are FP8, with actual group-32 activation casts. The
//! normalized query latent rounds to BF16 before FP8, matching norm_tl's
//! y_q_local conversion. Extra query-head norm and shared SWA hidden are
//! disabled by the pinned HF profile. Tail-64 RoPE follows both projections.
//! DeepInfer Apache-2.0 attn.py/mla_attn.py/norm.py at revision
//! 9bd9cf243fc53ef9afbf8f04d391d989117d6ab9. Unfused tensor control.

use aster_power::error::{PowerError, Result};
use candle_core::{DType, Tensor};
use std::sync::atomic::{AtomicU64, Ordering};
use tokio_util::sync::CancellationToken;

static NEXT_INPUT_OWNER: AtomicU64 = AtomicU64::new(1);

use crate::{
    layers::{
        invalid,
        kv::{MainKvFormat, MainKvRows, MAIN_KV_DIM},
        linear::{activation::Fp8Activations, DeviceMatrix},
        norm::EffectiveRmsNorm,
        rope::{RopeDirection, RopeLayout, RotaryTransform},
        tensor_error, CudaLayerDevice,
    },
    weights::MatrixLayout,
};

pub struct ProjectedAttentionInput {
    pub(super) owner: u64,
    query: Tensor,
    normalized_query: Fp8Activations,
    window: MainKvRows,
    positions: Vec<usize>,
}

impl ProjectedAttentionInput {
    pub fn query(&self) -> &Tensor {
        &self.query
    }
    pub fn normalized_query(&self) -> &Fp8Activations {
        &self.normalized_query
    }
    pub fn window_rows(&self) -> &MainKvRows {
        &self.window
    }
    pub fn positions(&self) -> &[usize] {
        &self.positions
    }
}

pub struct AttentionInputProjection {
    pub(super) owner: u64,
    device: CudaLayerDevice,
    query_a: DeviceMatrix,
    query_b: DeviceMatrix,
    window: DeviceMatrix,
    query_norm: EffectiveRmsNorm,
    window_norm: EffectiveRmsNorm,
    pub(super) rope: RotaryTransform,
    heads: usize,
    resident_bytes: u64,
}

impl AttentionInputProjection {
    pub fn new(
        device: CudaLayerDevice,
        query_a: DeviceMatrix,
        query_b: DeviceMatrix,
        window: DeviceMatrix,
        query_norm: EffectiveRmsNorm,
        window_norm: EffectiveRmsNorm,
        cos_sin: Tensor,
    ) -> Result<Self> {
        let [rank, hidden] = query_a.logical_shape();
        let heads = query_b.logical_shape()[0] / MAIN_KV_DIM;
        if !(1..=64).contains(&heads)
            || !query_b.logical_shape()[0].is_multiple_of(MAIN_KV_DIM)
            || query_b.logical_shape()[1] != rank
            || window.logical_shape() != [MAIN_KV_DIM, hidden]
            || !rank.is_multiple_of(32)
            || rank > 1280
            || !hidden.is_multiple_of(32)
            || hidden > 5120
            || query_norm.width() != rank
            || window_norm.width() != MAIN_KV_DIM
            || !query_norm.has_effective_weight()
            || !window_norm.has_effective_weight()
        {
            return Err(invalid("attention input requires complete FP8 Q-A/Q-B/SWA geometry, group-32 rank/hidden, 1..=64 width-512 heads and learned latent/KV norms"));
        }
        let mut bytes = 0_u64;
        for matrix in [&query_a, &query_b, &window] {
            validate_matrix(&device, matrix)?;
            bytes = add(bytes, matrix.retained_weight_bytes())?;
        }
        for norm in [&query_norm, &window_norm] {
            validate_owner(&device, norm.device())?;
            bytes = add(bytes, norm.retained_parameter_bytes())?;
        }
        let rope = main_rope(&device, cos_sin)?;
        let resident_bytes = resident(&device, add(bytes, rope.retained_cache_bytes())?)?;
        let owner = NEXT_INPUT_OWNER
            .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |id| id.checked_add(1))
            .map_err(|_| invalid("attention input owner sequence exhausted"))?;
        Ok(Self {
            owner,
            device,
            query_a,
            query_b,
            window,
            query_norm,
            window_norm,
            rope,
            heads,
            resident_bytes,
        })
    }
    pub fn retained_bytes(&self) -> u64 {
        self.resident_bytes
    }
    pub fn heads(&self) -> usize {
        self.heads
    }
    pub fn hidden_size(&self) -> usize {
        self.query_a.logical_shape()[1]
    }
    pub(crate) fn device(&self) -> &CudaLayerDevice {
        &self.device
    }

    pub fn forward(
        &self,
        current_hidden: &Tensor,
        positions: &[usize],
        cancellation: &CancellationToken,
    ) -> Result<ProjectedAttentionInput> {
        check_cancel(cancellation)?;
        let (tokens, hidden) = current_hidden.dims2().map_err(tensor_error)?;
        if current_hidden.dtype() != DType::BF16
            || hidden != self.hidden_size()
            || !current_hidden
                .device()
                .same_device(self.device.tensor_device())
        {
            return Err(invalid(
                "attention hidden input differs from its bound BF16 geometry/device",
            ));
        }
        validate_positions(&self.device, &self.rope, tokens, positions)?;
        self.device.limits().checked_state_bytes(
            self.live_tensor_bytes(tokens)?,
            "attention input composite live tensors",
        )?;
        let hidden = Fp8Activations::encode(&self.device, current_hidden, cancellation)?;
        let query_a = self
            .query_a
            .forward_fp8(&hidden, DType::BF16, cancellation)?;
        let normalized = self.query_norm.forward(&query_a)?;
        let normalized_query = Fp8Activations::encode(&self.device, &normalized, cancellation)?;
        let query = self
            .query_b
            .forward_fp8(&normalized_query, DType::BF16, cancellation)?
            .reshape((tokens, self.heads, MAIN_KV_DIM))
            .map_err(tensor_error)?;
        let query = self
            .rope
            .forward(&query, positions, RopeDirection::Forward, cancellation)?;
        // The pinned HF profile projects every layer's own current hidden.
        let window = self
            .window
            .forward_fp8(&hidden, DType::BF16, cancellation)?;
        let window = self
            .window_norm
            .forward(&window)?
            .unsqueeze(1)
            .map_err(tensor_error)?;
        let window = self
            .rope
            .forward(&window, positions, RopeDirection::Forward, cancellation)?
            .squeeze(1)
            .map_err(tensor_error)?;
        let window =
            MainKvRows::encode(&self.device, MainKvFormat::Fp8Ue8m0, &window, cancellation)?;
        check_cancel(cancellation)?;
        Ok(ProjectedAttentionInput {
            owner: self.owner,
            query,
            normalized_query,
            window,
            positions: positions.to_vec(),
        })
    }

    pub(crate) fn live_tensor_bytes(&self, tokens: usize) -> Result<u64> {
        let limits = self.device.limits();
        let input = limits
            .checked_elements(&[tokens, self.hidden_size()], "attention hidden activation")?
            as u64;
        let latent = limits.checked_elements(
            &[tokens, self.query_a.logical_shape()[0]],
            "attention query latent",
        )? as u64;
        let projected = limits.checked_elements(
            &[tokens, self.heads + 1, MAIN_KV_DIM],
            "attention Q and SWA activation",
        )? as u64;
        let tile =
            (self.hidden_size().max(self.query_a.logical_shape()[0]) as u64).checked_mul(32 * 64);
        let bytes = input
            .checked_add(latent)
            .and_then(|n| n.checked_mul(160))
            .zip(projected.checked_mul(256))
            .and_then(|(a, b)| a.checked_add(b))
            .zip(tile)
            .and_then(|(a, b)| a.checked_add(b))
            .and_then(|n| n.checked_add(self.resident_bytes))
            .and_then(|n| n.checked_add(4096))
            .ok_or_else(|| invalid("attention input live tensor bytes overflowed"))?;
        Ok(bytes)
    }
}

pub(super) fn main_rope(device: &CudaLayerDevice, cache: Tensor) -> Result<RotaryTransform> {
    if cache.dims().get(1) != Some(&64) {
        return Err(invalid(
            "main attention RoPE requires explicit FP32 rotary width 64",
        ));
    }
    RotaryTransform::from_cache(
        device.clone(),
        MAIN_KV_DIM,
        MAIN_KV_DIM - 64,
        RopeLayout::Interleaved,
        cache,
    )
}
pub(super) fn validate_matrix(device: &CudaLayerDevice, matrix: &DeviceMatrix) -> Result<()> {
    validate_owner(device, matrix.device())?;
    if matrix.layout() != MatrixLayout::Fp8Block32
        || matrix.source_first_row() != 0
        || matrix.source_total_rows() != matrix.logical_shape()[0]
    {
        return Err(invalid(
            "attention projection requires a complete FP8 block-32 matrix",
        ));
    }
    Ok(())
}
pub(crate) fn validate_owner(device: &CudaLayerDevice, component: &CudaLayerDevice) -> Result<()> {
    if !component
        .tensor_device()
        .same_device(device.tensor_device())
        || component.limits() != device.limits()
    {
        return Err(invalid(
            "attention component differs from its owning device/limits",
        ));
    }
    Ok(())
}
pub(super) fn validate_positions(
    device: &CudaLayerDevice,
    rope: &RotaryTransform,
    tokens: usize,
    positions: &[usize],
) -> Result<()> {
    if tokens == 0
        || tokens > device.limits().max_context_tokens
        || tokens != positions.len()
        || positions.iter().any(|p| *p >= rope.context_tokens())
    {
        return Err(invalid(
            "attention projection requires nonempty bounded rows and explicit in-cache positions",
        ));
    }
    let bytes = tokens
        .checked_mul(8)
        .ok_or_else(|| invalid("attention position bytes overflowed"))?;
    if bytes > device.limits().max_input_bytes {
        return Err(invalid("attention positions exceed metadata input budget"));
    }
    Ok(())
}
pub(crate) fn add(a: u64, b: u64) -> Result<u64> {
    a.checked_add(b)
        .ok_or_else(|| invalid("attention resident byte budget overflowed"))
}
pub(crate) fn resident(device: &CudaLayerDevice, bytes: u64) -> Result<u64> {
    if bytes > device.limits().max_resident_weight_bytes {
        return Err(invalid(
            "attention projection exceeds aggregate resident bytes",
        ));
    }
    Ok(bytes)
}
pub(crate) fn check_cancel(cancellation: &CancellationToken) -> Result<()> {
    if cancellation.is_cancelled() {
        Err(PowerError::InferenceCancelled)
    } else {
        Ok(())
    }
}

#[cfg(test)]
mod tests;
