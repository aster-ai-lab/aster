//! Shifted mHC -> learned norm -> complete native attention -> mHC post.
//! Preparation exposes the exact current hidden/query/window for the model's
//! compressor/index/cache stage; finishing never commits those state owners.
//! Rust adaptation of DeepInfer hc.py/hc_blocks.py (Apache-2.0), revision
//! 9bd9cf243fc53ef9afbf8f04d391d989117d6ab9.

use aster_power::error::Result;
use candle_core::Tensor;
use tokio_util::sync::CancellationToken;

use super::{
    branch::NativeAttention,
    projection::{add, check_cancel, resident, validate_owner, ProjectedAttentionInput},
    SparseAttentionPlan,
};
use crate::layers::{
    invalid,
    kv::MainKvRows,
    mhc::{MhcBlock, MhcContinuation, MhcStep},
    norm::EffectiveRmsNorm,
    tensor_error, CudaLayerDevice,
};

pub struct AttentionSublayer {
    device: CudaLayerDevice,
    mhc: MhcBlock,
    norm: EffectiveRmsNorm,
    attention: NativeAttention,
    resident_bytes: u64,
}

impl AttentionSublayer {
    pub(crate) fn initial_shifted(&self, embedding: &Tensor) -> Result<MhcContinuation> {
        self.mhc.initial_shifted(embedding)
    }

    pub(crate) fn device(&self) -> &CudaLayerDevice {
        &self.device
    }
    pub(crate) fn hidden_size(&self) -> usize {
        self.mhc.hidden_size()
    }
    pub(crate) fn prepare_live_tensor_bytes(&self, tokens: usize) -> Result<u64> {
        add(
            self.trunk_live_tensor_bytes(tokens)?,
            self.attention.projection_live_tensor_bytes(tokens)?,
        )
    }
    pub(crate) fn finish_live_tensor_bytes(
        &self,
        tokens: usize,
        plan: &SparseAttentionPlan,
    ) -> Result<u64> {
        add(
            self.trunk_live_tensor_bytes(tokens)?,
            self.attention.live_tensor_bytes(tokens, plan)?,
        )
    }
    pub fn new(
        device: CudaLayerDevice,
        mhc: MhcBlock,
        norm: EffectiveRmsNorm,
        attention: NativeAttention,
    ) -> Result<Self> {
        if mhc.hidden_size() != norm.width()
            || norm.width() != attention.hidden_size()
            || !norm.has_effective_weight()
        {
            return Err(invalid("attention sublayer requires matching mHC/norm/attention widths and explicit effective norm weights"));
        }
        for owner in [mhc.device(), norm.device(), attention.device()] {
            validate_owner(&device, owner)?;
        }
        let resident_bytes = resident(
            &device,
            add(
                add(
                    mhc.retained_parameter_bytes(),
                    norm.retained_parameter_bytes(),
                )?,
                attention.retained_bytes(),
            )?,
        )?;
        Ok(Self {
            device,
            mhc,
            norm,
            attention,
            resident_bytes,
        })
    }

    pub fn retained_bytes(&self) -> u64 {
        self.resident_bytes
    }

    pub fn prepare(
        &self,
        continuation: MhcContinuation,
        positions: &[usize],
        cancellation: &CancellationToken,
    ) -> Result<PreparedAttentionSublayer> {
        check_cancel(cancellation)?;
        let (tokens, _, width) = continuation.residual().dims3().map_err(tensor_error)?;
        if positions.len() != tokens || width != self.mhc.hidden_size() {
            return Err(invalid(
                "attention trunk/position count differs from its bound geometry",
            ));
        }
        let bytes = self.prepare_live_tensor_bytes(tokens)?;
        self.device
            .limits()
            .checked_state_bytes(bytes, "attention sublayer prepare live tensors")?;
        let step = self.mhc.pre_shifted(continuation)?;
        check_cancel(cancellation)?;
        let current_hidden = self.norm.forward(step.input())?;
        let projected = self
            .attention
            .project(&current_hidden, positions, cancellation)?;
        check_cancel(cancellation)?;
        Ok(PreparedAttentionSublayer {
            step,
            current_hidden,
            projected,
        })
    }

    /// The caller stages/retains actual KV/index views between prepare/finish,
    /// then registers *all* model participants for a later atomic commit.
    pub fn finish(
        &self,
        prepared: PreparedAttentionSublayer,
        window: &MainKvRows,
        compressed: Option<&MainKvRows>,
        plan: &SparseAttentionPlan,
        cancellation: &CancellationToken,
    ) -> Result<MhcContinuation> {
        check_cancel(cancellation)?;
        let tokens = prepared.projected.positions().len();
        self.device.limits().checked_state_bytes(
            self.finish_live_tensor_bytes(tokens, plan)?,
            "attention sublayer finish live tensors",
        )?;
        let branch =
            self.attention
                .attend(prepared.projected, window, compressed, plan, cancellation)?;
        let continued = prepared.step.finish_shifted(&branch)?;
        check_cancel(cancellation)?;
        Ok(continued)
    }

    fn trunk_live_tensor_bytes(&self, tokens: usize) -> Result<u64> {
        let norm_bytes = (tokens as u64)
            .checked_mul(self.norm.width() as u64)
            .and_then(|n| n.checked_mul(32))
            .ok_or_else(|| invalid("attention sublayer norm budget overflowed"))?;
        add(
            add(self.mhc.live_tensor_bytes(tokens)?, norm_bytes)?,
            self.resident_bytes,
        )
    }
}

pub struct PreparedAttentionSublayer {
    step: MhcStep,
    current_hidden: Tensor,
    projected: ProjectedAttentionInput,
}

impl PreparedAttentionSublayer {
    /// Same normalized current hidden for main compression and index weights.
    pub fn current_hidden(&self) -> &Tensor {
        &self.current_hidden
    }
    /// Packed normalized query is reused by the index query projection.
    pub fn projected(&self) -> &ProjectedAttentionInput {
        &self.projected
    }
}

#[cfg(test)]
pub(crate) mod tests;
