//! Complete XQ shifted attention half: F32 mHC/norm, original Q8 projections,
//! selected packed KV with the explicit F16 boundary and grouped output.
//! State preparation/publication remains the model transaction's job.

use aster_power::error::Result;
use candle_core::{DType, Tensor};
use std::sync::Arc;
use tokio_util::sync::CancellationToken;

use super::{
    admit, XqAttentionInputProjection, XqAttentionOutputProjection, XqProjectedAttentionInput,
};
use crate::{
    layers::{
        attention::{
            projection::{add, resident, validate_owner},
            SparseAttentionPlan, SparseMqaAttention,
        },
        kv::{MainKvRows, MAIN_KV_DIM},
        tensor_error,
        xq::{
            check_cancel, invalid,
            mhc::{XqMhcBlock, XqMhcContinuation, XqMhcSite, XqMhcStep},
            same_device, XqRmsNorm,
        },
        CudaLayerDevice,
    },
    weights::xq::{storage::XqWeights, XqTensorType},
};

pub struct XqAttentionSublayer {
    device: CudaLayerDevice,
    mhc: XqMhcBlock,
    norm: XqRmsNorm,
    input: XqAttentionInputProjection,
    output: XqAttentionOutputProjection,
    attention: SparseMqaAttention,
    resident_bytes: u64,
}

pub struct XqPreparedAttentionSublayer {
    step: XqMhcStep,
    current_hidden: Tensor,
    projected: XqProjectedAttentionInput,
}

impl XqPreparedAttentionSublayer {
    pub fn current_hidden(&self) -> &Tensor {
        &self.current_hidden
    }
    pub fn projected(&self) -> &XqProjectedAttentionInput {
        &self.projected
    }
}

impl XqAttentionSublayer {
    pub fn load(
        device: &CudaLayerDevice,
        weights: &XqWeights,
        layer: usize,
        cos_sin: Tensor,
        cancellation: &CancellationToken,
    ) -> Result<Self> {
        check_cancel(cancellation)?;
        let prefix = super::prefix(layer)?;
        let expected = [
            ("attn_q_a.weight", XqTensorType::Q8_0, vec![5120, 1280]),
            ("attn_q_b.weight", XqTensorType::Q8_0, vec![1280, 32768]),
            ("attn_kv.weight", XqTensorType::Q8_0, vec![5120, 512]),
            ("attn_q_a_norm.weight", XqTensorType::F32, vec![1280]),
            ("attn_kv_a_norm.weight", XqTensorType::F32, vec![512]),
            ("attn_output_a.weight", XqTensorType::Q8_0, vec![4096, 8192]),
            ("attn_output_b.weight", XqTensorType::Q8_0, vec![8192, 5120]),
            ("attn_norm.weight", XqTensorType::F32, vec![5120]),
            ("attn_sinks.weight", XqTensorType::F32, vec![64]),
            ("hc_attn_fn.weight", XqTensorType::F16, vec![20480, 24]),
            ("hc_attn_scale.weight", XqTensorType::F32, vec![3]),
            ("hc_attn_base.weight", XqTensorType::F32, vec![24]),
        ];
        // Input and output own separate immutable rotary snapshots.
        let admitted = resident(
            device,
            add(
                admit(device, weights, &prefix, &expected, &cos_sin)?,
                cos_sin.elem_count() as u64 * 4,
            )?,
        )?;
        let mhc = XqMhcBlock::load(device, weights, layer, XqMhcSite::Attention, cancellation)?;
        let norm = XqRmsNorm::load(
            device,
            weights,
            &format!("{prefix}.attn_norm.weight"),
            5120,
            cancellation,
        )?;
        let input = XqAttentionInputProjection::load(
            device,
            weights,
            layer,
            cos_sin.clone(),
            cancellation,
        )?;
        let output =
            XqAttentionOutputProjection::load(device, weights, layer, cos_sin, cancellation)?;
        let sinks = weights.load_f32_parameter(
            device,
            &format!("{prefix}.attn_sinks.weight"),
            XqTensorType::F32,
            &[64],
            cancellation,
        )?;
        let result = Self::new(device.clone(), mhc, norm, input, output, sinks)?;
        if result.retained_bytes() != admitted {
            return Err(invalid(
                "XQ attention half retained bytes differ from pre-upload admission",
            ));
        }
        check_cancel(cancellation)?;
        Ok(result)
    }

    pub fn new(
        device: CudaLayerDevice,
        mhc: XqMhcBlock,
        norm: XqRmsNorm,
        input: XqAttentionInputProjection,
        output: XqAttentionOutputProjection,
        sinks: Tensor,
    ) -> Result<Self> {
        let hidden = mhc.hidden_size();
        if hidden != norm.width()
            || hidden != input.hidden_size()
            || hidden != output.hidden_size()
            || input.heads() != output.heads
        {
            return Err(invalid(
                "XQ attention mHC/norm/input/output geometries differ",
            ));
        }
        for owner in [mhc.device(), norm.device(), &input.device, &output.device] {
            validate_owner(&device, owner)?;
        }
        let resident_bytes = resident(
            &device,
            add(
                add(mhc.retained_bytes(), norm.retained_bytes())?,
                add(
                    add(input.retained_bytes(), output.retained_bytes())?,
                    input.heads() as u64 * 4,
                )?,
            )?,
        )?;
        let cache_validation = (input.rope.retained_cache_bytes())
            .checked_add(output.rope.retained_cache_bytes())
            .and_then(|n| n.checked_mul(16))
            .and_then(|n| n.checked_add(resident_bytes))
            .ok_or_else(|| invalid("XQ paired rotary validation bytes overflowed"))?;
        device.limits().checked_state_bytes(
            cache_validation,
            "XQ paired rotary snapshots and parameters",
        )?;
        if !input.rope.same_cache_values(&output.rope)? {
            return Err(invalid("XQ main and conjugate output rotary caches differ"));
        }
        let attention = SparseMqaAttention::new(device.clone(), input.heads(), MAIN_KV_DIM, sinks)?;
        Ok(Self {
            device,
            mhc,
            norm,
            input,
            output,
            attention,
            resident_bytes,
        })
    }

    pub fn hidden_size(&self) -> usize {
        self.norm.width()
    }
    pub fn retained_bytes(&self) -> u64 {
        self.resident_bytes
    }
    pub(crate) fn device(&self) -> &CudaLayerDevice {
        &self.device
    }

    pub fn prepare(
        &self,
        continuation: XqMhcContinuation,
        positions: &[usize],
        cancellation: &CancellationToken,
    ) -> Result<XqPreparedAttentionSublayer> {
        check_cancel(cancellation)?;
        same_device(&self.device, continuation.residual())?;
        let (tokens, streams, hidden) = continuation.residual().dims3().map_err(tensor_error)?;
        if streams != 4
            || hidden != self.hidden_size()
            || continuation.residual().dtype() != DType::F32
        {
            return Err(invalid(
                "XQ attention trunk differs from its F32 four-stream geometry",
            ));
        }
        super::positions_valid(&self.device, &self.input.rope, tokens, positions)?;
        self.device.limits().checked_state_bytes(
            self.prepare_live_tensor_bytes(tokens)?,
            "XQ complete shifted attention prepare",
        )?;
        let step = self.mhc.prepare(continuation, cancellation)?;
        let current_hidden = self.norm.forward(step.input(), cancellation)?;
        let projected = self
            .input
            .forward(&current_hidden, positions, cancellation)?;
        check_cancel(cancellation)?;
        Ok(XqPreparedAttentionSublayer {
            step,
            current_hidden,
            projected,
        })
    }

    pub fn finish(
        &self,
        prepared: XqPreparedAttentionSublayer,
        window: &MainKvRows,
        compressed: Option<&MainKvRows>,
        plan: &SparseAttentionPlan,
        cancellation: &CancellationToken,
    ) -> Result<XqMhcContinuation> {
        check_cancel(cancellation)?;
        if !Arc::ptr_eq(&prepared.projected.owner, &self.input.owner)
            || prepared.projected.positions() != plan.query_positions()
        {
            return Err(invalid(
                "XQ prepared attention projection owner or query positions differ",
            ));
        }
        self.device.limits().checked_state_bytes(
            self.finish_live_tensor_bytes(plan)?,
            "XQ complete shifted attention finish",
        )?;
        let context = self.attention.forward_xq_f32_packed(
            prepared.projected.query(),
            window,
            compressed,
            plan,
            cancellation,
        )?;
        let branch = self
            .output
            .forward(&context, prepared.projected.positions(), cancellation)?;
        prepared.step.finish(&branch, cancellation)
    }

    pub(crate) fn prepare_live_tensor_bytes(&self, tokens: usize) -> Result<u64> {
        self.device.limits().checked_elements(
            &[tokens, self.hidden_size()],
            "XQ attention normalized hidden",
        )?;
        let width = self.hidden_size() as u64 * 128
            + self.input.query_a.logical_shape()[0] as u64 * 128
            + (self.input.heads() * MAIN_KV_DIM) as u64 * 128
            + MAIN_KV_DIM as u64 * 256;
        let mhc_bytes = self.mhc.live_tensor_bytes(tokens)?;
        let bytes = (tokens as u64)
            .checked_mul(width)
            .and_then(|n| n.checked_add(mhc_bytes))
            .and_then(|n| n.checked_add(self.resident_bytes))
            .ok_or_else(|| invalid("XQ attention prepare live bytes overflowed"))?;
        Ok(bytes)
    }

    pub(crate) fn finish_live_tensor_bytes(&self, plan: &SparseAttentionPlan) -> Result<u64> {
        let tokens = plan.query_positions().len();
        let widths = (self.input.heads() * MAIN_KV_DIM) as u64 * 128
            + (self.output.groups * self.output.rank) as u64 * 96
            + self.hidden_size() as u64 * 64;
        let projected = (tokens as u64)
            .checked_mul(widths)
            .ok_or_else(|| invalid("XQ output/attention live bytes overflowed"))?;
        add(
            add(self.prepare_live_tensor_bytes(tokens)?, projected)?,
            plan.live_tensor_bytes(self.input.heads(), MAIN_KV_DIM)?,
        )
    }
}

#[cfg(test)]
pub(crate) mod tests;
