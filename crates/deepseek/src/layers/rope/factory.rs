//! Official 64-channel FP32 RoPE cache construction. Rust adaptation of
//! DeepInfer's Apache-2.0 rotary_embedding.py at revision
//! 9bd9cf243fc53ef9afbf8f04d391d989117d6ab9. Original vLLM/EleutherAI/
//! HuggingFace notices are retained in licenses/Rotary-Apache-NOTICE.txt.
//!
//! FP32 phases use direct powf, reciprocal, explicit YaRN operations and
//! cos/sin before amplitude scaling (one for this model). Tensor::pow would
//! substitute exp(log(x)*y), and is intentionally not used. CPU fixtures
//! establish arithmetic order; they do not qualify CUDA libdevice results.

use aster_power::error::Result;
use candle_core::Tensor;
use tokio_util::sync::CancellationToken;

use super::check_cancelled;
use crate::{
    layers::{invalid, tensor_error, validate_finite, CudaLayerDevice},
    model::{topology::AttentionRopeProfile, MAX_CONTEXT_TOKENS},
};

const ROTARY_DIM: usize = 64;
const FREQUENCIES: usize = ROTARY_DIM / 2;
// Pinned HF profile: theta=160000, original context=65536, beta_fast=32,
// beta_slow=1. Python-double correction-range calculation floors/ceils to
// [15,25]. All tensor frequency/ramp/phase arithmetic below remains FP32.
const YARN_LOW: f32 = 15.;
const YARN_HIGH: f32 = 25.;
const YARN_FACTOR: f32 = 16.;

pub struct AttentionRopeCache {
    profile: AttentionRopeProfile,
    inverse_frequencies: Tensor,
    cos_sin: Tensor,
    context: usize,
}

impl AttentionRopeCache {
    pub fn new(
        device: &CudaLayerDevice,
        profile: AttentionRopeProfile,
        context: usize,
        cancellation: &CancellationToken,
    ) -> Result<Self> {
        check_cancelled(cancellation)?;
        if context == 0
            || context > device.limits().max_context_tokens
            || context > MAX_CONTEXT_TOKENS
        {
            return Err(invalid(
                "official rotary cache context exceeds its positive model/runtime bound",
            ));
        }
        let elements = device
            .limits()
            .checked_elements(&[context, ROTARY_DIM], "official rotary cache")?
            as u64;
        let retained = elements
            .checked_mul(4)
            .and_then(|n| n.checked_add((FREQUENCIES * 4) as u64))
            .ok_or_else(|| invalid("official rotary cache retained bytes overflowed"))?;
        if retained > device.limits().max_resident_weight_bytes {
            return Err(invalid(
                "official rotary cache exceeds its resident byte bound",
            ));
        }
        let working = elements
            .checked_mul(32)
            .and_then(|n| {
                (context as u64)
                    .checked_mul(8)
                    .and_then(|m| n.checked_add(m))
            })
            .and_then(|n| n.checked_add(32768))
            .ok_or_else(|| invalid("official rotary cache construction budget overflowed"))?;
        device
            .limits()
            .checked_state_bytes(working, "official rotary cache construction live tensors")?;
        let inverse_frequencies = inverse_frequencies(device, profile, cancellation)?;
        validate_finite(
            &inverse_frequencies,
            true,
            "official rotary inverse frequencies",
        )?;
        check_cancelled(cancellation)?;
        // All integers through this model's 2^20 context are exactly FP32.
        let positions =
            Tensor::arange(0_f32, context as f32, device.tensor_device()).map_err(tensor_error)?;
        let cos_sin = cache_rows(&inverse_frequencies, &positions)
            .map_err(tensor_error)?
            .detach();
        check_cancelled(cancellation)?;
        validate_finite(&cos_sin, false, "official rotary cos/sin cache")?;
        check_cancelled(cancellation)?;
        Ok(Self {
            profile,
            inverse_frequencies,
            cos_sin,
            context,
        })
    }

    pub fn profile(&self) -> AttentionRopeProfile {
        self.profile
    }
    pub fn context_tokens(&self) -> usize {
        self.context
    }
    pub fn inverse_frequencies(&self) -> &Tensor {
        &self.inverse_frequencies
    }
    pub fn cos_sin(&self) -> &Tensor {
        &self.cos_sin
    }
    pub fn retained_bytes(&self) -> u64 {
        ((self.inverse_frequencies.elem_count() + self.cos_sin.elem_count()) * 4) as u64
    }
}

fn inverse_frequencies(
    device: &CudaLayerDevice,
    profile: AttentionRopeProfile,
    cancellation: &CancellationToken,
) -> Result<Tensor> {
    let base =
        Tensor::full(profile.theta() as f32, 1, device.tensor_device()).map_err(tensor_error)?;
    let mut powers = Vec::with_capacity(FREQUENCIES);
    for pair in 0..FREQUENCIES {
        check_cancelled(cancellation)?;
        let exponent = (2 * pair) as f32 / ROTARY_DIM as f32;
        // Candle's scalar-powf API takes f64 but its F32 CPU/CUDA kernel
        // casts the exponent back to f32. These dyadic exponents are exact;
        // neither base powers nor inverse frequencies are computed in FP64.
        powers.push(base.powf(f64::from(exponent)).map_err(tensor_error)?);
    }
    let pos_freqs = Tensor::cat(&powers, 0).map_err(tensor_error)?;
    let extrapolated = pos_freqs.recip().map_err(tensor_error)?;
    if profile == AttentionRopeProfile::WindowBase {
        return Ok(extrapolated);
    }
    let compute = || -> candle_core::Result<Tensor> {
        let factor = Tensor::full(YARN_FACTOR, FREQUENCIES, device.tensor_device())?;
        let interpolated = pos_freqs.mul(&factor)?.recip()?;
        let one = Tensor::ones(FREQUENCIES, candle_core::DType::F32, device.tensor_device())?;
        let low = Tensor::full(YARN_LOW, FREQUENCIES, device.tensor_device())?;
        let width = Tensor::full(YARN_HIGH - YARN_LOW, FREQUENCIES, device.tensor_device())?;
        // Separate subtraction/division/multiplication/addition matches the
        // reference's rounding boundaries; do not fold this into affine/FMA.
        let ramp = Tensor::arange(0_f32, FREQUENCIES as f32, device.tensor_device())?
            .sub(&low)?
            .div(&width)?
            .clamp(0_f32, 1_f32)?;
        let mask = one.sub(&ramp)?;
        interpolated
            .mul(&one.sub(&mask)?)?
            .add(&extrapolated.mul(&mask)?)
    };
    let inverse = compute().map_err(tensor_error)?;
    check_cancelled(cancellation)?;
    Ok(inverse)
}

fn cache_rows(inverse: &Tensor, positions: &Tensor) -> candle_core::Result<Tensor> {
    let phases = positions
        .unsqueeze(1)?
        .broadcast_mul(&inverse.unsqueeze(0)?)?;
    // Reference torch.polar(ones, phases) precedes mscale. The frozen HF
    // profile sets both YaRN magnitude factors false, so amplitude is one.
    Tensor::cat(&[phases.cos()?, phases.sin()?], 1)
}

#[cfg(test)]
mod tests;
