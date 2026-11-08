//! Native XQ F32 state/numerics, distinct from official BF16/FP8 layers.
//! Adapted from Xolotl ds41_dense.cpp at
//! b3452666e07a6374d06bb8c2185dffa271e8f65f, MIT notice retained at
//! licenses/Xolotl-MIT.txt. Components do not register a complete model.

use aster_power::error::{PowerError, Result};
use candle_core::{DType, Tensor};
use tokio_util::sync::CancellationToken;

use super::{tensor_error, validate_finite, CudaLayerDevice};
use crate::weights::xq::{storage::XqWeights, XqTensorType};

pub mod attention;
pub mod compressor;
pub mod engram;
pub mod indexer;
pub mod mhc;
pub mod moe;
pub mod quant;
pub mod text;

/// Exact exported F32 RMS weights, with an F32 input/output boundary.
pub struct XqRmsNorm {
    device: CudaLayerDevice,
    weight: Tensor,
    width: usize,
}

impl XqRmsNorm {
    pub fn load(
        device: &CudaLayerDevice,
        weights: &XqWeights,
        name: &str,
        width: usize,
        cancellation: &CancellationToken,
    ) -> Result<Self> {
        let parameter = weights.load_f32_parameter(
            device,
            name,
            XqTensorType::F32,
            &[width as u64],
            cancellation,
        )?;
        Self::new(device.clone(), parameter)
    }

    pub fn new(device: CudaLayerDevice, weight: Tensor) -> Result<Self> {
        let width = weight.dims1().map_err(tensor_error)?;
        if width == 0
            || width > 20480
            || weight.dtype() != DType::F32
            || !weight.device().same_device(device.tensor_device())
        {
            return Err(invalid(
                "XQ norm requires a same-device finite F32 vector of width 1..20480",
            ));
        }
        device
            .limits()
            .checked_elements(&[width], "XQ norm weight")?;
        if width as u64 * 4 > device.limits().max_resident_weight_bytes {
            return Err(invalid("XQ norm weight exceeds its resident budget"));
        }
        device.limits().checked_state_bytes(
            width as u64 * 64,
            "XQ norm parameter snapshot and validation",
        )?;
        let weight = weight.force_contiguous().map_err(tensor_error)?.detach();
        validate_finite(&weight, false, "XQ norm weight")?;
        Ok(Self {
            device,
            weight,
            width,
        })
    }

    pub fn forward(&self, input: &Tensor, cancellation: &CancellationToken) -> Result<Tensor> {
        check_cancel(cancellation)?;
        if input.dims().last() != Some(&self.width)
            || !input.device().same_device(self.device.tensor_device())
        {
            return Err(invalid("XQ norm input differs from its bound width/device"));
        }
        self.device
            .limits()
            .checked_elements(input.dims(), "XQ RMS input")?;
        let live = (input.elem_count() as u64)
            .checked_mul(32)
            .and_then(|bytes| bytes.checked_add(self.retained_bytes()))
            .ok_or_else(|| invalid("XQ norm live bytes overflowed"))?;
        self.device
            .limits()
            .checked_state_bytes(live, "XQ RMS F32 live tensors")?;
        let result = rms_math(input, Some(&self.weight))?;
        check_cancel(cancellation)?;
        Ok(result)
    }

    pub fn width(&self) -> usize {
        self.width
    }
    pub fn retained_bytes(&self) -> u64 {
        self.width as u64 * 4
    }
    pub(crate) fn device(&self) -> &CudaLayerDevice {
        &self.device
    }
}

fn rms_math(input: &Tensor, weight: Option<&Tensor>) -> Result<Tensor> {
    let width = *input
        .dims()
        .last()
        .ok_or_else(|| invalid("XQ RMS requires a feature dimension"))?;
    if input.dtype() != DType::F32 || input.elem_count() == 0 || width == 0 || width > 20480 {
        return Err(invalid(
            "XQ RMS requires nonempty F32 data of width 1..20480",
        ));
    }
    if let Some(weight) = weight {
        if weight.dtype() != DType::F32
            || weight.dims() != [width]
            || !weight.device().same_device(input.device())
        {
            return Err(invalid(
                "XQ RMS effective weight differs from its F32 feature axis/device",
            ));
        }
        validate_finite(weight, false, "XQ RMS effective weight")?;
    }
    validate_finite(input, false, "XQ RMS input")?;
    let axis = input.rank() - 1;
    let square = input
        .sqr()
        .and_then(|x| x.mean_keepdim(axis))
        .and_then(|x| x.affine(1., 1e-20))
        .map_err(tensor_error)?;
    validate_finite(&square, false, "XQ RMS squared mean")?;
    let output = input
        .broadcast_div(&square.sqrt().map_err(tensor_error)?)
        .map_err(tensor_error)?;
    let output = match weight {
        Some(weight) => output.broadcast_mul(weight).map_err(tensor_error)?,
        None => output,
    };
    validate_finite(&output, false, "XQ RMS output")?;
    Ok(output.detach())
}

pub(crate) fn same_device(device: &CudaLayerDevice, tensor: &Tensor) -> Result<()> {
    if !tensor.device().same_device(device.tensor_device()) {
        return Err(invalid(
            "XQ component tensor differs from its admitted device",
        ));
    }
    Ok(())
}

pub(crate) fn check_cancel(cancellation: &CancellationToken) -> Result<()> {
    if cancellation.is_cancelled() {
        return Err(PowerError::InferenceCancelled);
    }
    Ok(())
}

pub(crate) fn invalid(reason: &str) -> PowerError {
    PowerError::InvalidRequest(reason.into())
}

#[cfg(test)]
mod tests;
