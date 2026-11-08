//! Shifted mHC -> learned effective RMSNorm -> full MoE -> mHC post.
//! This is the complete feed-forward half of a transformer layer. Attention,
//! Engram placement, checkpoint binding and model iteration are separate owners.
//! Composition follows DeepInfer's Apache-2.0 hc_blocks.py at revision
//! 9bd9cf243fc53ef9afbf8f04d391d989117d6ab9.

use aster_power::error::{PowerError, Result};
use candle_core::Tensor;
use tokio_util::sync::CancellationToken;

use super::{
    invalid,
    mhc::{MhcBlock, MhcContinuation},
    moe::block::MoeBlock,
    norm::EffectiveRmsNorm,
    tensor_error, CudaLayerDevice,
};

pub struct MoeSublayer {
    device: CudaLayerDevice,
    mhc: MhcBlock,
    norm: EffectiveRmsNorm,
    moe: MoeBlock,
    resident_bytes: u64,
}

impl MoeSublayer {
    pub(crate) fn device(&self) -> &CudaLayerDevice {
        &self.device
    }
    pub(crate) fn hidden_size(&self) -> usize {
        self.mhc.hidden_size()
    }

    pub fn new(
        device: CudaLayerDevice,
        mhc: MhcBlock,
        norm: EffectiveRmsNorm,
        moe: MoeBlock,
    ) -> Result<Self> {
        if mhc.hidden_size() != norm.width()
            || mhc.hidden_size() != moe.hidden_size()
            || !norm.has_effective_weight()
        {
            return Err(invalid("MoE sublayer requires matching mHC/norm/MoE widths and explicit effective norm weights"));
        }
        for owner in [mhc.device(), norm.device(), moe.device()] {
            if !owner.tensor_device().same_device(device.tensor_device())
                || owner.limits() != device.limits()
            {
                return Err(invalid(
                    "MoE sublayer components differ from its owning device/limits",
                ));
            }
        }
        let resident_bytes = mhc
            .retained_parameter_bytes()
            .checked_add(norm.retained_parameter_bytes())
            .and_then(|n| n.checked_add(moe.retained_weight_bytes()))
            .ok_or_else(|| invalid("MoE sublayer resident bytes overflowed"))?;
        if resident_bytes > device.limits().max_resident_weight_bytes {
            return Err(invalid(
                "MoE sublayer exceeds aggregate resident parameter bytes",
            ));
        }
        Ok(Self {
            device,
            mhc,
            norm,
            moe,
            resident_bytes,
        })
    }
    pub fn retained_weight_bytes(&self) -> u64 {
        self.resident_bytes
    }

    pub fn forward(
        &self,
        continuation: MhcContinuation,
        image_mask: Option<&Tensor>,
        cancellation: &CancellationToken,
    ) -> Result<MhcContinuation> {
        check_cancel(cancellation)?;
        let (tokens, _, width) = continuation.residual().dims3().map_err(tensor_error)?;
        if width != self.mhc.hidden_size() {
            return Err(invalid(
                "MoE sublayer trunk width differs from its bound geometry",
            ));
        }
        self.live_tensor_bytes(tokens)?;
        let step = self.mhc.pre_shifted(continuation)?;
        check_cancel(cancellation)?;
        let normalized = self.norm.forward(step.input())?;
        check_cancel(cancellation)?;
        let branch = self.moe.forward(&normalized, image_mask, cancellation)?;
        let continued = step.finish_shifted(&branch)?;
        check_cancel(cancellation)?;
        Ok(continued)
    }

    pub(crate) fn live_tensor_bytes(&self, tokens: usize) -> Result<u64> {
        let norm_bytes = (tokens as u64)
            .checked_mul(self.hidden_size() as u64)
            .and_then(|n| n.checked_mul(32))
            .ok_or_else(|| invalid("MoE sublayer norm budget overflowed"))?;
        let bytes = self
            .mhc
            .live_tensor_bytes(tokens)?
            .checked_add(self.moe.live_tensor_bytes(tokens)?)
            .and_then(|n| n.checked_add(norm_bytes))
            .ok_or_else(|| invalid("MoE sublayer live tensor bytes overflowed"))?;
        self.device
            .limits()
            .checked_state_bytes(bytes, "MoE sublayer composite live tensors")
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
