//! Actual packed sparse embeddings -> FP8 fused-KV projection -> BF16 gate.
//! Engram replaces the trunk while preserving the previous shifted mHC mix.
//! DeepInfer algorithm adaptation (Apache-2.0), revision
//! 9bd9cf243fc53ef9afbf8f04d391d989117d6ab9, v4_modules/engram.py.

use aster_power::error::{PowerError, Result};
use candle_core::DType;
use tokio_util::sync::CancellationToken;

use super::{
    lookup::{EngramEmbeddings, PROJECTED_INPUT_WIDTH},
    EngramGate,
};
use crate::{
    layers::{invalid, linear::DeviceMatrix, mhc::MhcContinuation, tensor_error, CudaLayerDevice},
    weights::MatrixLayout,
};

pub struct EngramProjection {
    device: CudaLayerDevice,
    projection: DeviceMatrix,
    gate: EngramGate,
    resident_bytes: u64,
}

impl EngramProjection {
    pub(crate) fn device(&self) -> &CudaLayerDevice {
        &self.device
    }
    pub(crate) fn hidden_size(&self) -> usize {
        self.gate.hidden_size()
    }
    pub fn new(
        device: CudaLayerDevice,
        projection: DeviceMatrix,
        gate: EngramGate,
    ) -> Result<Self> {
        if projection.layout() != MatrixLayout::Fp8Block32
            || projection.logical_shape() != [5 * gate.hidden_size(), PROJECTED_INPUT_WIDTH]
            || projection.source_first_row() != 0
            || projection.source_total_rows() != 5 * gate.hidden_size()
        {
            return Err(invalid(
                "Engram fused projection requires complete FP8 block-32 [5*hidden,6144] weights",
            ));
        }
        for owner in [projection.device(), gate.device()] {
            if !owner.tensor_device().same_device(device.tensor_device())
                || owner.limits() != device.limits()
            {
                return Err(invalid("Engram projection/gate device or limits differ"));
            }
        }
        let resident_bytes = projection
            .retained_weight_bytes()
            .checked_add(gate.retained_parameter_bytes())
            .ok_or_else(|| invalid("Engram resident parameter bytes overflowed"))?;
        if resident_bytes > device.limits().max_resident_weight_bytes {
            return Err(invalid(
                "Engram projection exceeds aggregate resident parameter budget",
            ));
        }
        Ok(Self {
            device,
            projection,
            gate,
            resident_bytes,
        })
    }

    pub fn retained_weight_bytes(&self) -> u64 {
        self.resident_bytes
    }

    pub fn forward(
        &self,
        continuation: MhcContinuation,
        embeddings: &EngramEmbeddings,
        cancellation: &CancellationToken,
    ) -> Result<MhcContinuation> {
        check_cancel(cancellation)?;
        let (tokens, streams, width) = continuation.residual().dims3().map_err(tensor_error)?;
        if streams != 4
            || width != self.gate.hidden_size()
            || embeddings.activations().data_bytes().dims() != [tokens, PROJECTED_INPUT_WIDTH]
            || !embeddings
                .device()
                .tensor_device()
                .same_device(self.device.tensor_device())
            || embeddings.device().limits() != self.device.limits()
            || !continuation
                .residual()
                .device()
                .same_device(self.device.tensor_device())
        {
            return Err(invalid(
                "Engram trunk/embeddings differ from the bound geometry/device/limits",
            ));
        }
        self.live_tensor_bytes(tokens, embeddings.live_tensor_bytes())?;
        let projected = self
            .projection
            .forward_fp8(embeddings.activations(), DType::BF16, cancellation)?
            .reshape((tokens, 5, width))
            .map_err(tensor_error)?;
        check_cancel(cancellation)?;
        let residual = self.gate.forward(
            continuation.residual(),
            &projected,
            Some(embeddings.image_mask()),
        )?;
        let continued = continuation.with_residual(&residual)?;
        check_cancel(cancellation)?;
        Ok(continued)
    }

    pub(crate) fn live_tensor_bytes(&self, tokens: usize, lookup_bytes: u64) -> Result<u64> {
        let bytes = self
            .projection
            .live_tensor_bytes(tokens)?
            .checked_add(self.gate.live_tensor_bytes(tokens)?)
            .and_then(|n| n.checked_add(lookup_bytes))
            .and_then(|n| n.checked_add(self.resident_bytes))
            .ok_or_else(|| invalid("Engram composite live budget overflowed"))?;
        self.device
            .limits()
            .checked_state_bytes(bytes, "Engram projection/gate/lookup live tensors")
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
pub(crate) mod tests;
