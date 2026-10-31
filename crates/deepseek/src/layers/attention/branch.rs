//! Complete projected main-attention branch over explicitly prepared packed
//! request KV views. Rust owns Q/SWA projections, sparse MQA and grouped Wo.
//! The model builds candidate/index plans, registers every state participant,
//! and stages/commits the projected window rows; this branch never commits KV.

use aster_power::error::Result;
use candle_core::Tensor;
use tokio_util::sync::CancellationToken;

use super::{
    attention_peak_bytes,
    output::AttentionOutputProjection,
    projection::{
        add, check_cancel, resident, validate_owner, AttentionInputProjection,
        ProjectedAttentionInput,
    },
    SparseAttentionPlan, SparseMqaAttention,
};
use crate::layers::{
    invalid,
    kv::{MainKvRows, MAIN_KV_DIM},
    CudaLayerDevice,
};

pub struct NativeAttention {
    device: CudaLayerDevice,
    input: AttentionInputProjection,
    attention: SparseMqaAttention,
    output: AttentionOutputProjection,
    resident_bytes: u64,
}

impl NativeAttention {
    pub(crate) fn device(&self) -> &CudaLayerDevice {
        &self.device
    }

    pub(crate) fn projection_live_tensor_bytes(&self, tokens: usize) -> Result<u64> {
        self.input.live_tensor_bytes(tokens)
    }

    pub fn new(
        input: AttentionInputProjection,
        output: AttentionOutputProjection,
        sink_logits: Tensor,
    ) -> Result<Self> {
        let device = input.device().clone();
        validate_owner(&device, output.device())?;
        if input.heads() != output.heads() || input.hidden_size() != output.hidden_size() {
            return Err(invalid(
                "native attention input/output head and hidden geometry differs",
            ));
        }
        let attention =
            SparseMqaAttention::new(device.clone(), input.heads(), MAIN_KV_DIM, sink_logits)?;
        let resident_bytes = resident(
            &device,
            add(
                add(input.retained_bytes(), output.retained_bytes())?,
                input.heads() as u64 * 4,
            )?,
        )?;
        device.limits().checked_state_bytes(
            resident_bytes
                .checked_mul(16)
                .ok_or_else(|| invalid("native attention construction budget overflowed"))?,
            "native attention construction tensors",
        )?;
        if !input.rope.same_cache_values(&output.rope)? {
            return Err(invalid(
                "native attention Q/SWA and conjugate output RoPE caches differ",
            ));
        }
        Ok(Self {
            device,
            input,
            attention,
            output,
            resident_bytes,
        })
    }
    pub fn hidden_size(&self) -> usize {
        self.input.hidden_size()
    }
    pub fn heads(&self) -> usize {
        self.input.heads()
    }
    pub fn retained_bytes(&self) -> u64 {
        self.resident_bytes
    }
    pub fn project(
        &self,
        current_hidden: &Tensor,
        positions: &[usize],
        cancellation: &CancellationToken,
    ) -> Result<ProjectedAttentionInput> {
        self.input.forward(current_hidden, positions, cancellation)
    }

    /// Window includes the prepared current rows; compressed is the source
    /// layer's prepared/read-only cache view. The supplied plan determines
    /// the exact causal/index selection. No rejected state is published here.
    pub fn attend(
        &self,
        projected: ProjectedAttentionInput,
        window: &MainKvRows,
        compressed: Option<&MainKvRows>,
        plan: &SparseAttentionPlan,
        cancellation: &CancellationToken,
    ) -> Result<Tensor> {
        check_cancel(cancellation)?;
        if projected.owner != self.input.owner || projected.positions() != plan.query_positions() {
            return Err(invalid(
                "native attention projection owner or query positions differ from its plan",
            ));
        }
        let tokens = projected.positions().len();
        self.device.limits().checked_state_bytes(
            self.live_tensor_bytes(tokens, plan)?,
            "native attention composite live tensors",
        )?;
        let context = self.attention.forward_packed(
            projected.query(),
            window,
            compressed,
            plan,
            cancellation,
        )?;
        self.output
            .forward(&context, projected.positions(), cancellation)
    }

    pub(crate) fn live_tensor_bytes(
        &self,
        tokens: usize,
        plan: &SparseAttentionPlan,
    ) -> Result<u64> {
        add(
            add(
                self.input.live_tensor_bytes(tokens)?,
                self.output.live_tensor_bytes(tokens)?,
            )?,
            attention_peak_bytes(
                tokens,
                self.heads(),
                MAIN_KV_DIM,
                plan.kv_rows,
                plan.max_selected,
            )?,
        )
    }
}

#[cfg(test)]
pub(crate) mod tests;
