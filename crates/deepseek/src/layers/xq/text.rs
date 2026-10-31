//! Exact XQ F16 token embedding and F32 shifted-head -> norm -> Q8_0 logits.
//! These endpoints complement the layer owner; they do not bypass the trunk.

use aster_power::error::Result;
use candle_core::Tensor;
use tokio_util::sync::CancellationToken;

use super::mhc::XqMhcContinuation;
use super::{check_cancel, invalid, same_device, XqRmsNorm};
use crate::cuda::xq::XqProjectionReceipt;
use crate::layers::{linear::xq::XqDeviceMatrix, tensor_error, CudaLayerDevice};
use crate::weights::xq::{storage::XqWeights, XqTensorType};

pub struct XqTokenEmbedding {
    device: CudaLayerDevice,
    matrix: XqDeviceMatrix,
}

impl XqTokenEmbedding {
    pub fn load(
        device: &CudaLayerDevice,
        weights: &XqWeights,
        cancellation: &CancellationToken,
    ) -> Result<Self> {
        let tensor = weights.tensor("token_embd.weight")?;
        if tensor.kind != XqTensorType::F16 || tensor.shape != [5120, 129280] {
            return Err(invalid(
                "XQ embedding differs from pinned F16[5120,129280] storage",
            ));
        }
        Self::new(
            device.clone(),
            XqDeviceMatrix::load(
                device,
                weights,
                "token_embd.weight",
                0,
                129280,
                cancellation,
            )?,
        )
    }

    pub fn new(device: CudaLayerDevice, matrix: XqDeviceMatrix) -> Result<Self> {
        let [vocabulary, hidden] = matrix.logical_shape();
        if matrix.kind() != XqTensorType::F16
            || vocabulary == 0
            || vocabulary > 129280
            || hidden == 0
            || hidden > 5120
            || !device
                .tensor_device()
                .same_device(matrix.device().tensor_device())
            || device.limits() != matrix.device().limits()
        {
            return Err(invalid(
                "XQ embedding requires same-device packed F16 [1..129280,1..5120] rows",
            ));
        }
        if matrix.retained_weight_bytes() > device.limits().max_resident_weight_bytes {
            return Err(invalid("XQ embedding exceeds its admitted resident budget"));
        }
        Ok(Self { device, matrix })
    }

    pub fn hidden_size(&self) -> usize {
        self.matrix.logical_shape()[1]
    }
    pub fn vocabulary_size(&self) -> usize {
        self.matrix.logical_shape()[0]
    }
    pub fn retained_bytes(&self) -> u64 {
        self.matrix.retained_weight_bytes()
    }

    pub fn forward(
        &self,
        tokens: &[u32],
        cancellation: &CancellationToken,
    ) -> Result<(Tensor, XqProjectionReceipt)> {
        check_cancel(cancellation)?;
        if tokens.is_empty()
            || tokens
                .iter()
                .any(|&id| id as usize >= self.vocabulary_size())
        {
            return Err(invalid(
                "XQ embedding token IDs are empty or outside its bound vocabulary",
            ));
        }
        self.device
            .limits()
            .checked_elements(&[tokens.len(), self.hidden_size()], "XQ embedding output")?;
        let indices_bytes = (tokens.len() as u64)
            .checked_mul(8)
            .ok_or_else(|| invalid("XQ token index byte count overflowed"))?;
        self.device
            .limits()
            .checked_state_bytes(indices_bytes, "XQ immutable token indices and upload")?;
        let indices = Tensor::from_slice(tokens, tokens.len(), self.device.tensor_device())
            .map_err(tensor_error)?;
        self.matrix.gather_f32_rows(&indices, cancellation)
    }

    pub fn initial_continuation(
        &self,
        tokens: &[u32],
        cancellation: &CancellationToken,
    ) -> Result<XqMhcContinuation> {
        let (embedding, _) = self.forward(tokens, cancellation)?;
        XqMhcContinuation::from_embedding(&self.device, &embedding, cancellation)
    }
}

pub struct XqTextHead {
    device: CudaLayerDevice,
    norm: XqRmsNorm,
    output: XqDeviceMatrix,
}

impl XqTextHead {
    pub fn load(
        device: &CudaLayerDevice,
        weights: &XqWeights,
        cancellation: &CancellationToken,
    ) -> Result<Self> {
        let tensor = weights.tensor("output.weight")?;
        if tensor.kind != XqTensorType::Q8_0 || tensor.shape != [5120, 129280] {
            return Err(invalid(
                "XQ output head differs from pinned Q8_0[5120,129280] storage",
            ));
        }
        let norm = XqRmsNorm::load(device, weights, "output_norm.weight", 5120, cancellation)?;
        Self::new(
            device.clone(),
            norm,
            XqDeviceMatrix::load(device, weights, "output.weight", 0, 129280, cancellation)?,
        )
    }

    pub fn new(device: CudaLayerDevice, norm: XqRmsNorm, output: XqDeviceMatrix) -> Result<Self> {
        let [vocabulary, hidden] = output.logical_shape();
        if output.kind() != XqTensorType::Q8_0
            || hidden != norm.width()
            || hidden > 5120
            || vocabulary == 0
            || vocabulary > 129280
            || !device
                .tensor_device()
                .same_device(norm.device().tensor_device())
            || !device
                .tensor_device()
                .same_device(output.device().tensor_device())
            || device.limits() != norm.device().limits()
            || device.limits() != output.device().limits()
        {
            return Err(invalid(
                "XQ head requires matching F32 norm and packed Q8_0 output geometry/device",
            ));
        }
        let retained = output
            .retained_weight_bytes()
            .checked_add(norm.retained_bytes())
            .ok_or_else(|| invalid("XQ head retained bytes overflowed"))?;
        if retained > device.limits().max_resident_weight_bytes {
            return Err(invalid("XQ head exceeds its admitted resident budget"));
        }
        Ok(Self {
            device,
            norm,
            output,
        })
    }

    pub fn vocabulary_size(&self) -> usize {
        self.output.logical_shape()[0]
    }
    pub fn retained_bytes(&self) -> u64 {
        self.output.retained_weight_bytes() + self.norm.retained_bytes()
    }

    pub fn forward(
        &self,
        continuation: &XqMhcContinuation,
        cancellation: &CancellationToken,
    ) -> Result<(Tensor, XqProjectionReceipt)> {
        check_cancel(cancellation)?;
        same_device(&self.device, continuation.residual())?;
        let (tokens, streams, hidden) = continuation.residual().dims3().map_err(tensor_error)?;
        if streams != 4 || hidden != self.norm.width() {
            return Err(invalid(
                "XQ output head continuation differs from its four-stream hidden geometry",
            ));
        }
        self.device
            .limits()
            .checked_elements(&[tokens, self.vocabulary_size()], "XQ vocabulary logits")?;
        let live = (tokens as u64)
            .checked_mul((hidden * 80 + self.vocabulary_size() * 64) as u64)
            .ok_or_else(|| invalid("XQ head live tensor bound overflowed"))?;
        self.device
            .limits()
            .checked_state_bytes(live, "XQ head reduction, norm and logits")?;
        let hidden = continuation.head_input()?;
        let normalized = self.norm.forward(&hidden, cancellation)?;
        self.output.forward_f32_control(&normalized, cancellation)
    }
}

#[cfg(test)]
mod tests;
