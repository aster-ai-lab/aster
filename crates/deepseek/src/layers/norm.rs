//! Effective-weight RMSNorm tensor control. HF weights already contain their
//! exported scaling; no exponentiation or additional output scale is inferred.
//! Math follows DeepInfer's Apache-2.0 norm/runtime contract at revision
//! 9bd9cf243fc53ef9afbf8f04d391d989117d6ab9.

use aster_power::error::Result;
use candle_core::{DType, Tensor};

use super::{invalid, tensor_error, validate_finite, CudaLayerDevice};

const MAX_NORM_WIDTH: usize = 20_480;

pub struct EffectiveRmsNorm {
    device: CudaLayerDevice,
    width: usize,
    epsilon: f64,
    effective_weight: Option<Tensor>,
}

impl EffectiveRmsNorm {
    pub(crate) fn device(&self) -> &CudaLayerDevice {
        &self.device
    }
    pub(crate) fn width(&self) -> usize {
        self.width
    }
    pub(crate) fn has_effective_weight(&self) -> bool {
        self.effective_weight.is_some()
    }
    pub(crate) fn retained_parameter_bytes(&self) -> u64 {
        self.effective_weight
            .as_ref()
            .map_or(0, |weight| weight.elem_count() as u64 * 4)
    }
    pub fn new(
        device: CudaLayerDevice,
        width: usize,
        epsilon: f64,
        effective_weight: Option<Tensor>,
    ) -> Result<Self> {
        validate_parameters(width, epsilon)?;
        device
            .limits()
            .checked_elements(&[width], "RMSNorm weight")?;
        let effective_weight = effective_weight
            .as_ref()
            .map(|weight| freeze_weight(weight, width, device.tensor_device()))
            .transpose()?;
        Ok(Self {
            device,
            width,
            epsilon,
            effective_weight,
        })
    }

    pub fn forward(&self, input: &Tensor) -> Result<Tensor> {
        if input.dims().last() != Some(&self.width)
            || !input.device().same_device(self.device.tensor_device())
        {
            return Err(invalid(
                "RMSNorm input differs from its bound CUDA width/device",
            ));
        }
        self.device
            .limits()
            .checked_elements(input.dims(), "RMSNorm input")?;
        // Named live tensors only; Power must add runtime/allocator overhead
        // and aggregate the model-wide reservation before serving activation.
        let bytes = (input.elem_count() as u64)
            .checked_mul(24)
            .and_then(|bytes| bytes.checked_add((self.width as u64) * 4))
            .ok_or_else(|| invalid("RMSNorm tensor-byte estimate overflowed"))?;
        self.device
            .limits()
            .checked_state_bytes(bytes, "RMSNorm live tensors")?;
        rms_math(input, self.effective_weight.as_ref(), self.epsilon)
    }
}

fn validate_parameters(width: usize, epsilon: f64) -> Result<()> {
    if width == 0
        || width > MAX_NORM_WIDTH
        || !epsilon.is_finite()
        || (epsilon as f32) <= 0.0
        || epsilon > f32::MAX as f64
    {
        return Err(invalid(
            "RMSNorm requires width 1..=20480 and positive finite FP32 epsilon",
        ));
    }
    Ok(())
}

fn freeze_weight(weight: &Tensor, width: usize, device: &candle_core::Device) -> Result<Tensor> {
    if weight.dims() != [width]
        || !matches!(weight.dtype(), DType::BF16 | DType::F32)
        || !weight.device().same_device(device)
    {
        return Err(invalid(
            "effective RMSNorm weights require a same-device BF16/FP32 width vector",
        ));
    }
    let weight = weight
        .force_contiguous()
        .and_then(|weight| weight.to_dtype(DType::F32))
        .map_err(tensor_error)?
        .detach();
    validate_finite(&weight, false, "effective RMSNorm weight")?;
    Ok(weight)
}

// CPU invocation is private to sibling tests. The public constructor requires
// an actual CUDA handle and stores immutable effective FP32 weights.
fn rms_math(input: &Tensor, effective_weight: Option<&Tensor>, epsilon: f64) -> Result<Tensor> {
    let width = input
        .dims()
        .last()
        .copied()
        .ok_or_else(|| invalid("RMSNorm input requires a feature dimension"))?;
    validate_parameters(width, epsilon)?;
    if input.elem_count() == 0 || !matches!(input.dtype(), DType::BF16 | DType::F32) {
        return Err(invalid("RMSNorm requires nonempty BF16/FP32 input"));
    }
    if let Some(weight) = effective_weight {
        if weight.dtype() != DType::F32
            || weight.dims() != [width]
            || !weight.device().same_device(input.device())
        {
            return Err(invalid(
                "RMSNorm effective weights differ from the input width/device",
            ));
        }
    }
    validate_finite(input, false, "RMSNorm input")?;
    let x = input.detach().to_dtype(DType::F32).map_err(tensor_error)?;
    let axis = input.rank() - 1;
    let denominator = x
        .sqr()
        .and_then(|value| value.mean_keepdim(axis))
        .and_then(|value| value.affine(1.0, epsilon))
        .map_err(tensor_error)?;
    validate_finite(&denominator, false, "RMSNorm squared mean")?;
    let normalize = || -> candle_core::Result<Tensor> {
        let normalized = x.broadcast_div(&denominator.sqrt()?)?;
        match effective_weight {
            Some(weight) => normalized.broadcast_mul(weight)?.to_dtype(DType::BF16),
            None => normalized.to_dtype(DType::BF16),
        }
    };
    let output = normalize().map_err(tensor_error)?.detach();
    validate_finite(&output, false, "RMSNorm output")?;
    Ok(output)
}

#[cfg(test)]
mod tests;
