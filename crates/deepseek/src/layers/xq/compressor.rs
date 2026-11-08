//! XQ F16 projections with F32 C1/C2 state, learned norm and explicit FP4
//! main-KV grid. Readiness uses the pair end; compressed RoPE uses its start.
//! Pinned ds41_dense.cpp / ds41_geometry.hpp, MIT notice retained.
use aster_power::error::Result;
use candle_core::{DType, Tensor};
use tokio_util::sync::CancellationToken;

use super::attention::{admit, rotary};
use super::quant::{XqActivationFormat, XqQuantizedActivation};
use super::{check_cancel, invalid, same_device, XqRmsNorm};
use crate::{
    layers::{
        attention::projection::{add, resident, validate_owner},
        kv::MainKvRows,
        linear::xq::XqDeviceMatrix,
        rope::{RopeDirection, RotaryTransform},
        tensor_error, CudaLayerDevice,
    },
    state::{CompressionKind, CompressionNumerics, CompressionState, PreparedCompression},
    weights::xq::{storage::XqWeights, XqTensorType},
};

pub struct XqPreparedCompressedKv {
    layer: usize,
    compression: PreparedCompression,
    pre_rope: Tensor,
    rows: MainKvRows,
    rotary_positions: Vec<usize>,
}
impl XqPreparedCompressedKv {
    pub fn layer(&self) -> usize {
        self.layer
    }
    /// Causal readiness, e.g. 1,3 for C2. It is not the compressed RoPE phase.
    pub fn emitted_positions(&self) -> &[usize] {
        self.compression.emitted_positions()
    }
    /// Group-start positions, e.g. 0,2 for C2, used by both main and index K.
    pub fn rotary_positions(&self) -> &[usize] {
        &self.rotary_positions
    }
    pub fn pre_rope_kv(&self) -> &Tensor {
        &self.pre_rope
    }
    pub fn rows(&self) -> &MainKvRows {
        &self.rows
    }
    pub fn into_parts(self) -> (PreparedCompression, Tensor, MainKvRows, Vec<usize>) {
        (
            self.compression,
            self.pre_rope,
            self.rows,
            self.rotary_positions,
        )
    }
}

pub struct XqKvCompressor {
    device: CudaLayerDevice,
    layer: usize,
    kind: CompressionKind,
    latent: XqDeviceMatrix,
    gate: Option<XqDeviceMatrix>,
    norm: XqRmsNorm,
    rope: RotaryTransform,
    resident_bytes: u64,
}

impl XqKvCompressor {
    pub fn load(
        device: &CudaLayerDevice,
        weights: &XqWeights,
        layer: usize,
        cos_sin: Tensor,
        cancellation: &CancellationToken,
    ) -> Result<Self> {
        check_cancel(cancellation)?;
        let kind = source_kind(layer)?;
        let prefix = format!("blk.{layer}");
        let mut expected = vec![
            (
                "attn_compressor_kv.weight",
                XqTensorType::F16,
                vec![5120, 512],
            ),
            ("attn_compressor_norm.weight", XqTensorType::F32, vec![512]),
        ];
        if kind == CompressionKind::C2 {
            expected.push((
                "attn_compressor_gate.weight",
                XqTensorType::F16,
                vec![5120, 512],
            ));
        }
        let admitted = admit(device, weights, &prefix, &expected, &cos_sin)?;
        let load = |suffix: &str| {
            XqDeviceMatrix::load(
                device,
                weights,
                &format!("{prefix}.{suffix}"),
                0,
                512,
                cancellation,
            )
        };
        let latent = load("attn_compressor_kv.weight")?;
        let gate = if kind == CompressionKind::C2 {
            Some(load("attn_compressor_gate.weight")?)
        } else {
            None
        };
        let norm = XqRmsNorm::load(
            device,
            weights,
            &format!("{prefix}.attn_compressor_norm.weight"),
            512,
            cancellation,
        )?;
        let output = Self::from_parts(device.clone(), layer, latent, gate, norm, cos_sin)?;
        if output.retained_bytes() != admitted {
            return Err(invalid(
                "XQ compressor retained bytes differ from pre-upload admission",
            ));
        }
        check_cancel(cancellation)?;
        Ok(output)
    }
    pub(super) fn from_parts(
        device: CudaLayerDevice,
        layer: usize,
        latent: XqDeviceMatrix,
        gate: Option<XqDeviceMatrix>,
        norm: XqRmsNorm,
        cos_sin: Tensor,
    ) -> Result<Self> {
        let kind = source_kind(layer)?;
        let shape = latent.logical_shape();
        if shape[0] != 512
            || shape[1] == 0
            || shape[1] > 5120
            || norm.width() != 512
            || (kind == CompressionKind::C2) != gate.is_some()
        {
            return Err(invalid("XQ compressor requires source-specific F16 A/Gate, width-512 F32 norm and complete hidden geometry"));
        }
        validate_owner(&device, norm.device())?;
        let mut bytes = norm.retained_bytes();
        for matrix in std::iter::once(&latent).chain(gate.as_ref()) {
            validate_f16(&device, matrix)?;
            if matrix.logical_shape() != shape {
                return Err(invalid("XQ compressor A/Gate shapes differ"));
            }
            bytes = add(bytes, matrix.retained_weight_bytes())?;
        }
        let rope = rotary(&device, &cos_sin, 512)?;
        let resident_bytes = resident(&device, add(bytes, rope.retained_cache_bytes())?)?;
        Ok(Self {
            device,
            layer,
            kind,
            latent,
            gate,
            norm,
            rope,
            resident_bytes,
        })
    }
    pub fn layer(&self) -> usize {
        self.layer
    }
    pub fn kind(&self) -> CompressionKind {
        self.kind
    }
    pub fn retained_bytes(&self) -> u64 {
        self.resident_bytes
    }
    pub fn new_state(&self, context: usize, step: usize) -> Result<CompressionState> {
        if context > self.rope.context_tokens() {
            return Err(invalid(
                "XQ compressor state context exceeds its rotary cache",
            ));
        }
        CompressionState::new_with_numerics(
            &self.device,
            self.kind,
            CompressionNumerics::XqF32,
            512,
            context,
            step,
        )
    }
    pub fn prepare(
        &self,
        state: &CompressionState,
        start: usize,
        hidden: &Tensor,
        cancellation: &CancellationToken,
    ) -> Result<XqPreparedCompressedKv> {
        check_cancel(cancellation)?;
        same_device(&self.device, hidden)?;
        let (tokens, width) = hidden.dims2().map_err(tensor_error)?;
        if hidden.dtype() != DType::F32 || width != self.latent.logical_shape()[1] {
            return Err(invalid("XQ compressor requires matching F32 hidden rows"));
        }
        self.validate_state_rows(state, start, tokens)?;
        self.live_tensor_bytes(state, tokens)?;
        let latent = self.latent.forward_f32_control(hidden, cancellation)?.0;
        let gate = self
            .gate
            .as_ref()
            .map(|gate| gate.forward_f32_control(hidden, cancellation).map(|x| x.0))
            .transpose()?;
        self.prepare_projected(state, start, gate.as_ref(), &latent, cancellation)
    }
    fn prepare_projected(
        &self,
        state: &CompressionState,
        start: usize,
        gate: Option<&Tensor>,
        latent: &Tensor,
        cancellation: &CancellationToken,
    ) -> Result<XqPreparedCompressedKv> {
        check_cancel(cancellation)?;
        self.validate_state_rows(state, start, latent.dim(0).map_err(tensor_error)?)?;
        self.live_tensor_bytes(state, latent.dim(0).map_err(tensor_error)?)?;
        let compression = state.prepare(start, gate, latent, cancellation)?;
        let rotary_positions =
            compressed_rotary_positions(self.kind, compression.emitted_positions())?;
        let pre_rope = match compression.emissions() {
            Some(rows) => self.norm.forward(rows, cancellation)?,
            None => Tensor::zeros((0, 512), DType::F32, self.device.tensor_device())
                .map_err(tensor_error)?,
        };
        let rotated = if rotary_positions.is_empty() {
            pre_rope.clone()
        } else {
            self.rope
                .forward_f32_control(
                    &pre_rope.unsqueeze(1).map_err(tensor_error)?,
                    &rotary_positions,
                    RopeDirection::Forward,
                    cancellation,
                )?
                .squeeze(1)
                .map_err(tensor_error)?
        };
        let rows = XqQuantizedActivation::encode(
            &self.device,
            XqActivationFormat::CompressedFp4,
            &rotated,
            cancellation,
        )?
        .into_main_rows(cancellation)?;
        check_cancel(cancellation)?;
        Ok(XqPreparedCompressedKv {
            layer: self.layer,
            compression,
            pre_rope,
            rows,
            rotary_positions,
        })
    }
    fn validate_state_rows(
        &self,
        state: &CompressionState,
        start: usize,
        tokens: usize,
    ) -> Result<()> {
        if state.numerics() != CompressionNumerics::XqF32
            || state.kind() != self.kind
            || state.width() != 512
            || !state.device().same_device(self.device.tensor_device())
            || state.limits() != self.device.limits()
            || tokens == 0
            || tokens > state.max_step_tokens()
            || start != state.position()
            || start
                .checked_add(tokens)
                .is_none_or(|end| end > self.rope.context_tokens())
        {
            return Err(invalid("XQ compressor state differs in F32 profile, source kind, device/limits, position or cache/step bound"));
        }
        Ok(())
    }
    pub(crate) fn live_tensor_bytes(&self, state: &CompressionState, tokens: usize) -> Result<u64> {
        let limits = self.device.limits();
        let hidden = limits.checked_elements(
            &[tokens, self.latent.logical_shape()[1]],
            "XQ compressor hidden",
        )? as u64;
        if tokens
            .checked_mul(16)
            .is_none_or(|bytes| bytes > limits.max_input_bytes)
        {
            return Err(invalid(
                "XQ compressor ready/rotary metadata exceeds host input budget",
            ));
        }
        let latent = limits.checked_elements(&[tokens + 2, 512], "XQ compressor latent")? as u64;
        let packed = self.latent.retained_weight_bytes().max(
            self.gate
                .as_ref()
                .map_or(0, XqDeviceMatrix::retained_weight_bytes),
        );
        let mut bytes = add(
            state.live_step_bytes(tokens)?,
            self.rope.retained_cache_bytes(),
        )?;
        for (count, factor) in [(hidden, 64), (latent, 256), (packed, 2)] {
            bytes = add(
                bytes,
                count
                    .checked_mul(factor)
                    .ok_or_else(|| invalid("XQ compressor live bytes overflowed"))?,
            )?;
        }
        limits.checked_state_bytes(bytes, "XQ compressor complete F32 live tensors")?;
        Ok(bytes)
    }
}

pub(super) fn source_kind(layer: usize) -> Result<CompressionKind> {
    match layer {
        2 | 8 | 14 => Ok(CompressionKind::C2),
        20 => Ok(CompressionKind::C1),
        _ => Err(invalid(
            "XQ compressor/index-K sources are exactly layers 2,8,14,20",
        )),
    }
}
pub(super) fn validate_f16(device: &CudaLayerDevice, matrix: &XqDeviceMatrix) -> Result<()> {
    validate_owner(device, matrix.device())?;
    if matrix.kind() != XqTensorType::F16 {
        return Err(invalid(
            "XQ compressor/index projection requires original F16 weights",
        ));
    }
    Ok(())
}
fn compressed_rotary_positions(kind: CompressionKind, ready: &[usize]) -> Result<Vec<usize>> {
    if ready.iter().any(|p| p % kind.ratio() != kind.ratio() - 1)
        || ready
            .windows(2)
            .any(|pair| pair[1].checked_sub(pair[0]) != Some(kind.ratio()))
    {
        return Err(invalid(
            "XQ compressed readiness must contain consecutive completed groups",
        ));
    }
    ready
        .iter()
        .map(|ready| {
            ready
                .checked_sub(kind.ratio() - 1)
                .ok_or_else(|| invalid("XQ compressed ready position precedes its group start"))
        })
        .collect()
}

#[cfg(test)]
pub(crate) mod tests;
