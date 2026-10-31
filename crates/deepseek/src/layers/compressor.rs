//! BF16 A/Gate projections -> transactional C1/C2 compression -> learned
//! RMSNorm -> trailing RoPE -> typed main KV. The post-norm/pre-RoPE latent
//! is retained explicitly for index-K; a quantized main-KV roundtrip cannot
//! replace it. C2 projections accumulate to F32 before per-channel softmax.
//! Apache-2.0 DeepInfer compress.py/attn.py at revision
//! 9bd9cf243fc53ef9afbf8f04d391d989117d6ab9. Rust owns every transaction.

use aster_power::error::{PowerError, Result};
use candle_core::{DType, Tensor};
use tokio_util::sync::CancellationToken;

use super::{
    invalid,
    kv::{MainKvFormat, MainKvRows, MAIN_KV_DIM},
    linear::DeviceMatrix,
    norm::EffectiveRmsNorm,
    rope::{RopeDirection, RopeLayout, RotaryTransform},
    tensor_error, CudaLayerDevice,
};
use crate::{
    state::{CompressionKind, CompressionNumerics, CompressionState, PreparedCompression},
    weights::MatrixLayout,
};

pub struct PreparedCompressedKv {
    compression: PreparedCompression,
    pre_rope: Tensor,
    rows: MainKvRows,
}

impl PreparedCompressedKv {
    pub fn emitted_positions(&self) -> &[usize] {
        self.compression.emitted_positions()
    }
    pub fn pre_rope_kv(&self) -> &Tensor {
        &self.pre_rope
    }
    pub fn rows(&self) -> &MainKvRows {
        &self.rows
    }
    /// The model stages this compression proposal alongside its main/index
    /// cache proposals in StateCommitBatch after computing speculative outputs.
    pub fn into_parts(self) -> (PreparedCompression, Tensor, MainKvRows) {
        (self.compression, self.pre_rope, self.rows)
    }
}

pub struct KvCompressor {
    device: CudaLayerDevice,
    kind: CompressionKind,
    latent: DeviceMatrix,
    gate: Option<DeviceMatrix>,
    norm: EffectiveRmsNorm,
    rope: RotaryTransform,
    format: MainKvFormat,
    resident_bytes: u64,
}

impl KvCompressor {
    pub fn new(
        device: CudaLayerDevice,
        kind: CompressionKind,
        latent: DeviceMatrix,
        gate: Option<DeviceMatrix>,
        norm: EffectiveRmsNorm,
        cos_sin: Tensor,
        format: MainKvFormat,
    ) -> Result<Self> {
        let shape = latent.logical_shape();
        if latent.layout() != MatrixLayout::Bf16
            || shape[0] != MAIN_KV_DIM
            || (kind == CompressionKind::C2) != gate.is_some()
            || norm.width() != MAIN_KV_DIM
            || !norm.has_effective_weight()
            || !norm
                .device()
                .tensor_device()
                .same_device(device.tensor_device())
            || norm.device().limits() != device.limits()
            || cos_sin.dims().get(1) != Some(&64)
        {
            return Err(invalid("main compressor requires BF16 A to width 512, Gate exactly for C2, learned same-device norm and explicit rotary width 64"));
        }
        let mut resident_bytes = norm.retained_parameter_bytes();
        for projection in std::iter::once(&latent).chain(gate.as_ref()) {
            if projection.layout() != MatrixLayout::Bf16
                || projection.logical_shape() != shape
                || projection.source_first_row() != 0
                || projection.source_total_rows() != shape[0]
                || !projection
                    .device()
                    .tensor_device()
                    .same_device(device.tensor_device())
                || projection.device().limits() != device.limits()
            {
                return Err(invalid("compressor projections require complete matching BF16 matrices on their owning device/limits"));
            }
            resident_bytes = resident_bytes
                .checked_add(projection.retained_weight_bytes())
                .ok_or_else(|| invalid("compressor resident byte budget overflowed"))?;
        }
        let rope = RotaryTransform::from_cache(
            device.clone(),
            MAIN_KV_DIM,
            MAIN_KV_DIM - 64,
            RopeLayout::Interleaved,
            cos_sin,
        )?;
        resident_bytes = resident_bytes
            .checked_add(rope.retained_cache_bytes())
            .ok_or_else(|| invalid("compressor resident byte budget overflowed"))?;
        if resident_bytes > device.limits().max_resident_weight_bytes {
            return Err(invalid("compressor exceeds aggregate resident bytes"));
        }
        Ok(Self {
            device,
            kind,
            latent,
            gate,
            norm,
            rope,
            format,
            resident_bytes,
        })
    }
    pub fn retained_bytes(&self) -> u64 {
        self.resident_bytes
    }
    pub fn kind(&self) -> CompressionKind {
        self.kind
    }

    pub fn prepare(
        &self,
        state: &CompressionState,
        start: usize,
        hidden: &Tensor,
        cancellation: &CancellationToken,
    ) -> Result<PreparedCompressedKv> {
        check_cancel(cancellation)?;
        let (tokens, width) = hidden.dims2().map_err(tensor_error)?;
        if hidden.dtype() != DType::BF16
            || width != self.latent.logical_shape()[1]
            || !hidden.device().same_device(self.device.tensor_device())
            || !state.device().same_device(self.device.tensor_device())
            || state.limits() != self.device.limits()
            || state.width() != MAIN_KV_DIM
            || state.kind() != self.kind
            || state.numerics() != CompressionNumerics::OfficialBf16
            || start != state.position()
            || tokens == 0
            || tokens > state.max_step_tokens()
            || start
                .checked_add(tokens)
                .is_none_or(|end| end > self.rope.context_tokens())
        {
            return Err(invalid("compressor input/state differs from its BF16 geometry, kind, device, limits, position or bounded cache/step"));
        }
        self.device
            .limits()
            .checked_elements(hidden.dims(), "compressor hidden input")?;
        self.check_live_bytes(state, tokens, width)?;
        let output_type = match self.kind {
            CompressionKind::C1 => DType::BF16,
            CompressionKind::C2 => DType::F32,
        };
        let latent = self
            .latent
            .forward_bf16(hidden, output_type, cancellation)?;
        let gate = self
            .gate
            .as_ref()
            .map(|gate| gate.forward_bf16(hidden, DType::F32, cancellation))
            .transpose()?;
        let compression = state.prepare(start, gate.as_ref(), &latent, cancellation)?;
        let pre_rope = match compression.emissions() {
            Some(emitted) => self.norm.forward(emitted)?,
            None => Tensor::zeros((0, MAIN_KV_DIM), DType::BF16, self.device.tensor_device())
                .map_err(tensor_error)?,
        };
        let rotated = if compression.emitted_positions().is_empty() {
            pre_rope.clone()
        } else {
            self.rope
                .forward(
                    &pre_rope.unsqueeze(1).map_err(tensor_error)?,
                    compression.emitted_positions(),
                    RopeDirection::Forward,
                    cancellation,
                )?
                .squeeze(1)
                .map_err(tensor_error)?
        };
        let rows = MainKvRows::encode(&self.device, self.format, &rotated, cancellation)?;
        check_cancel(cancellation)?;
        Ok(PreparedCompressedKv {
            compression,
            pre_rope,
            rows,
        })
    }

    fn check_live_bytes(
        &self,
        state: &CompressionState,
        tokens: usize,
        hidden: usize,
    ) -> Result<()> {
        let state_bytes = state.live_step_bytes(tokens)?;
        let input = (tokens as u64)
            .checked_mul(hidden as u64)
            .and_then(|n| n.checked_mul(32));
        let work = (tokens as u64)
            .checked_add(2)
            .and_then(|n| n.checked_mul(MAIN_KV_DIM as u64))
            .and_then(|n| n.checked_mul(200));
        let tile = (hidden as u64).checked_mul(32 * 64);
        let bytes = input
            .zip(work)
            .and_then(|(a, b)| a.checked_add(b))
            .zip(tile)
            .and_then(|(a, b)| a.checked_add(b))
            .and_then(|n| n.checked_add(state_bytes))
            .and_then(|n| n.checked_add(self.resident_bytes))
            .and_then(|n| n.checked_add(4096))
            .ok_or_else(|| invalid("compressor composite live bytes overflowed"))?;
        self.device
            .limits()
            .checked_state_bytes(bytes, "compressor composite live tensors")?;
        Ok(())
    }
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
