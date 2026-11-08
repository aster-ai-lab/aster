//! Engram signed-root gate, adapted to Rust/Candle from TileKernels'
//! MIT-licensed `tile_kernels/torch/engram.py`, revision
//! c1cf914a82e6901c679445f872051ce44ac2cc63 (Copyright 2026 DeepSeek).

use aster_power::error::Result;
use candle_core::{DType, Tensor};

use super::{invalid, tensor_error, validate_finite, CudaLayerDevice};

pub mod lookup;
pub mod projection;

pub struct EngramGate {
    device: CudaLayerDevice,
    hidden_weight: Tensor,
    key_weight: Tensor,
    epsilon: f64,
    clamp: f64,
    hidden: usize,
}

impl EngramGate {
    pub fn new(
        device: CudaLayerDevice,
        hidden_weight: Tensor,
        key_weight: Tensor,
        epsilon: f64,
        clamp: f64,
    ) -> Result<Self> {
        validate_numerics(epsilon, clamp)?;
        let (heads, hidden) = hidden_weight.dims2().map_err(tensor_error)?;
        if heads != 4 || hidden == 0 || hidden > 5120 {
            return Err(invalid(
                "Engram norm weights require four positive hidden streams",
            ));
        }
        device
            .limits
            .checked_elements(&[heads, hidden], "Engram norm weights")?;
        validate_weights(
            &hidden_weight,
            &key_weight,
            hidden,
            device.runtime.tensor_device(),
        )?;
        let resident = (hidden as u64) * 16;
        if resident > device.limits().max_resident_weight_bytes {
            return Err(invalid(
                "Engram norm weights exceed resident parameter budget",
            ));
        }
        device
            .limits()
            .checked_state_bytes(resident * 16 + 4096, "Engram parameter snapshot")?;
        let hidden_weight = hidden_weight
            .force_contiguous()
            .map_err(tensor_error)?
            .detach();
        let key_weight = key_weight
            .force_contiguous()
            .map_err(tensor_error)?
            .detach();
        validate_finite(&hidden_weight, false, "Engram hidden weights")?;
        validate_finite(&key_weight, false, "Engram key weights")?;
        Ok(Self {
            device,
            hidden_weight,
            key_weight,
            epsilon,
            clamp,
            hidden,
        })
    }

    pub(crate) fn device(&self) -> &CudaLayerDevice {
        &self.device
    }

    pub(crate) fn hidden_size(&self) -> usize {
        self.hidden
    }

    pub(crate) fn retained_parameter_bytes(&self) -> u64 {
        (self.hidden as u64) * 16
    }

    pub(crate) fn live_tensor_bytes(&self, tokens: usize) -> Result<u64> {
        self.device
            .limits()
            .checked_elements(&[tokens, 5, self.hidden], "Engram gate rows")?;
        let bytes = (tokens as u64)
            .checked_mul(self.hidden as u64)
            .and_then(|n| n.checked_mul(512))
            .and_then(|n| n.checked_add(self.retained_parameter_bytes() * 16 + 4096))
            .ok_or_else(|| invalid("Engram gate live budget overflowed"))?;
        self.device
            .limits()
            .checked_state_bytes(bytes, "Engram gate live tensors")
    }

    /// KV is the actual BF16 output of this layer's packed projection: four
    /// independent keys and one value shared by the four residual streams.
    pub fn forward(
        &self,
        residual: &Tensor,
        projected_kv: &Tensor,
        image_mask: Option<&Tensor>,
    ) -> Result<Tensor> {
        let (tokens, heads, hidden) = residual.dims3().map_err(tensor_error)?;
        if tokens == 0
            || heads != 4
            || hidden != self.hidden
            || !residual
                .device()
                .same_device(self.device.runtime.tensor_device())
        {
            return Err(invalid("Engram residual differs from its bound CUDA layer"));
        }
        self.live_tensor_bytes(tokens)?;
        gate_math(
            residual,
            projected_kv,
            &self.hidden_weight,
            &self.key_weight,
            self.epsilon,
            self.clamp,
            image_mask,
        )
    }
}

fn validate_numerics(epsilon: f64, clamp: f64) -> Result<()> {
    if [epsilon, clamp]
        .iter()
        .any(|value| !value.is_finite() || (*value as f32) <= 0.0 || *value > f32::MAX as f64)
    {
        return Err(invalid(
            "Engram epsilon and signed-root clamp require positive finite FP32 values",
        ));
    }
    Ok(())
}

fn validate_weights(
    hidden_weight: &Tensor,
    key_weight: &Tensor,
    hidden: usize,
    device: &candle_core::Device,
) -> Result<()> {
    for weight in [hidden_weight, key_weight] {
        if weight.dims() != [4, hidden]
            || weight.dtype() != DType::BF16
            || !weight.device().same_device(device)
        {
            return Err(invalid(
                "Engram norm weights require same-device BF16 [4,hidden] tensors",
            ));
        }
    }
    Ok(())
}

// As with mHC, the CPU entry is private to hermetic tests. The exported
// EngramGate can only be constructed with CudaLayerDevice.
fn gate_math(
    residual: &Tensor,
    projected_kv: &Tensor,
    hidden_weight: &Tensor,
    key_weight: &Tensor,
    epsilon: f64,
    clamp: f64,
    image_mask: Option<&Tensor>,
) -> Result<Tensor> {
    validate_numerics(epsilon, clamp)?;
    let (tokens, heads, hidden) = residual.dims3().map_err(tensor_error)?;
    if tokens == 0
        || heads != 4
        || hidden == 0
        || residual.dtype() != DType::BF16
        || projected_kv.dims() != [tokens, 5, hidden]
        || projected_kv.dtype() != DType::BF16
        || !projected_kv.device().same_device(residual.device())
    {
        return Err(invalid("Engram gate requires same-device BF16 residual [tokens,4,hidden] and KV [tokens,5,hidden]"));
    }
    validate_weights(hidden_weight, key_weight, hidden, residual.device())?;
    if image_mask.is_some_and(|mask| {
        mask.dims() != [tokens]
            || mask.dtype() != DType::U8
            || !mask.device().same_device(residual.device())
    }) {
        return Err(invalid(
            "Engram image mask requires same-device U8 [tokens] data",
        ));
    }
    if let Some(mask) = image_mask {
        if mask
            .max_all()
            .and_then(|t| t.to_scalar::<u8>())
            .map_err(tensor_error)?
            > 1
        {
            return Err(invalid("Engram image mask values must be zero or one"));
        }
    }
    validate_finite(residual, false, "Engram residual")?;
    validate_finite(projected_kv, false, "Engram projected KV")?;
    let compute = || -> candle_core::Result<Tensor> {
        let x = residual.to_dtype(DType::F32)?;
        let k = projected_kv.narrow(1, 0, 4)?.to_dtype(DType::F32)?;
        let value = projected_kv.narrow(1, 4, 1)?.to_dtype(DType::F32)?;
        let rstd_x = x
            .sqr()?
            .mean_keepdim(2)?
            .affine(1.0, epsilon)?
            .sqrt()?
            .recip()?;
        let rstd_k = k
            .sqr()?
            .mean_keepdim(2)?
            .affine(1.0, epsilon)?
            .sqrt()?
            .recip()?;
        let weighted_x = x.broadcast_mul(&hidden_weight.to_dtype(DType::F32)?.unsqueeze(0)?)?;
        let weighted_k = k.broadcast_mul(&key_weight.to_dtype(DType::F32)?.unsqueeze(0)?)?;
        let dot = weighted_x
            .mul(&weighted_k)?
            .sum_keepdim(2)?
            .mul(&rstd_x)?
            .mul(&rstd_k)?
            .affine((hidden as f64).powf(-0.5), 0.0)?;
        let signed_root = dot.abs()?.maximum(clamp)?.sqrt()?.mul(&dot.sign()?)?;
        let gate = candle_nn::ops::sigmoid(&signed_root)?;
        let output = x.add(&gate.broadcast_mul(&value)?)?.to_dtype(DType::BF16)?;
        match image_mask {
            Some(mask) => mask
                .reshape((tokens, 1, 1))?
                .broadcast_as((tokens, 4, hidden))?
                .where_cond(residual, &output),
            None => Ok(output),
        }
    };
    let output = compute().map_err(tensor_error)?;
    validate_finite(&output, false, "Engram gate output")?;
    Ok(output)
}

#[cfg(test)]
mod tests;
