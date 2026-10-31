//! Context -> conjugate tail RoPE -> actual FP8 input -> grouped Wv -> BF16
//! -> FP8 -> Wo -> BF16. Pinned DeepGEMM's fused FP8 output also rounds its
//! accumulator to BF16 before amax/cast; this control preserves that boundary.
//! Group-32 FP8 output/inputs in the pinned batched kernel require SM100;
//! none of those kernels are imported or claimed to execute on SM90 here.
//! DeepInfer Apache-2.0 attn.py at 9bd9cf243fc53ef9afbf8f04d391d989117d6ab9.

use aster_power::error::Result;
use candle_core::{DType, Tensor};
use tokio_util::sync::CancellationToken;

use super::projection::{
    add, check_cancel, main_rope, resident, validate_matrix, validate_positions,
};
use crate::layers::{
    invalid,
    kv::MAIN_KV_DIM,
    linear::{activation::Fp8Activations, DeviceMatrix},
    rope::{RopeDirection, RotaryTransform},
    tensor_error, CudaLayerDevice,
};

pub struct AttentionOutputProjection {
    device: CudaLayerDevice,
    groups: Vec<DeviceMatrix>,
    output: DeviceMatrix,
    pub(super) rope: RotaryTransform,
    heads: usize,
    rank: usize,
    group_width: usize,
    resident_bytes: u64,
}

impl AttentionOutputProjection {
    /// Wv rows are [groups * rank, heads/groups * 512]; each contiguous
    /// rank-row block consumes its corresponding context head group.
    pub fn new(
        device: CudaLayerDevice,
        group_count: usize,
        value: DeviceMatrix,
        output: DeviceMatrix,
        cos_sin: Tensor,
    ) -> Result<Self> {
        validate_matrix(&device, &value)?;
        validate_matrix(&device, &output)?;
        let [rows, group_width] = value.logical_shape();
        if !(1..=8).contains(&group_count)
            || !rows.is_multiple_of(group_count)
            || !group_width.is_multiple_of(MAIN_KV_DIM)
            || !(rows / group_count).is_multiple_of(32)
            || rows / group_count > 1024
            || group_width
                .checked_mul(group_count)
                .is_none_or(|n| n / MAIN_KV_DIM > 64)
            || output.logical_shape()[1] != rows
            || output.logical_shape()[0] > 5120
        {
            return Err(invalid("attention output requires 1..=8 groups, aligned rank <=1024, width-512 heads and matching complete FP8 output projection"));
        }
        let rank = rows / group_count;
        let heads = group_count * group_width / MAIN_KV_DIM;
        let rope = main_rope(&device, cos_sin)?;
        let resident_bytes = resident(
            &device,
            add(
                add(
                    value.retained_weight_bytes(),
                    output.retained_weight_bytes(),
                )?,
                rope.retained_cache_bytes(),
            )?,
        )?;
        device.limits().checked_state_bytes(
            resident_bytes
                .checked_mul(16)
                .ok_or_else(|| invalid("attention output construction budget overflowed"))?,
            "attention output grouped construction tensors",
        )?;
        let cancellation = CancellationToken::new();
        let groups = (0..group_count)
            .map(|g| value.fp8_output_rows(g * rank, rank, &cancellation))
            .collect::<Result<Vec<_>>>()?;
        Ok(Self {
            device,
            groups,
            output,
            rope,
            heads,
            rank,
            group_width,
            resident_bytes,
        })
    }
    pub fn retained_bytes(&self) -> u64 {
        self.resident_bytes
    }
    pub fn heads(&self) -> usize {
        self.heads
    }
    pub fn hidden_size(&self) -> usize {
        self.output.logical_shape()[0]
    }
    pub(crate) fn device(&self) -> &CudaLayerDevice {
        &self.device
    }

    pub fn forward(
        &self,
        context: &Tensor,
        positions: &[usize],
        cancellation: &CancellationToken,
    ) -> Result<Tensor> {
        check_cancel(cancellation)?;
        let (tokens, heads, width) = context.dims3().map_err(tensor_error)?;
        if context.dtype() != DType::BF16
            || heads != self.heads
            || width != MAIN_KV_DIM
            || !context.device().same_device(self.device.tensor_device())
        {
            return Err(invalid(
                "attention context differs from its bound BF16 head geometry/device",
            ));
        }
        validate_positions(&self.device, &self.rope, tokens, positions)?;
        self.device.limits().checked_state_bytes(
            self.live_tensor_bytes(tokens)?,
            "attention output composite live tensors",
        )?;
        let context = self
            .rope
            .forward(context, positions, RopeDirection::Conjugate, cancellation)?
            .reshape((tokens, heads * width))
            .map_err(tensor_error)?;
        let packed = Fp8Activations::encode(&self.device, &context, cancellation)?;
        let mut results = Vec::with_capacity(self.groups.len());
        for (group, weight) in self.groups.iter().enumerate() {
            let input =
                packed.column_range(group * self.group_width, self.group_width, cancellation)?;
            results.push(weight.forward_fp8(&input, DType::BF16, cancellation)?);
        }
        let intermediate = Tensor::cat(&results, 1).map_err(tensor_error)?;
        let intermediate = Fp8Activations::encode(&self.device, &intermediate, cancellation)?;
        self.output
            .forward_fp8(&intermediate, DType::BF16, cancellation)
    }

    pub(crate) fn live_tensor_bytes(&self, tokens: usize) -> Result<u64> {
        let limits = self.device.limits();
        let context = limits
            .checked_elements(&[tokens, self.heads, MAIN_KV_DIM], "attention context")?
            as u64;
        let mid = limits.checked_elements(
            &[tokens, self.groups.len() * self.rank],
            "attention grouped output",
        )? as u64;
        let out =
            limits.checked_elements(&[tokens, self.hidden_size()], "attention output")? as u64;
        let tile =
            (self.group_width.max(self.groups.len() * self.rank) as u64).checked_mul(32 * 64);
        context
            .checked_mul(256)
            .zip(mid.checked_mul(160))
            .and_then(|(a, b)| a.checked_add(b))
            .zip(out.checked_mul(32))
            .and_then(|(a, b)| a.checked_add(b))
            .zip(tile)
            .and_then(|(a, b)| a.checked_add(b))
            .and_then(|n| n.checked_add(self.resident_bytes))
            .and_then(|n| n.checked_add(4096))
            .ok_or_else(|| invalid("attention output live tensor bytes overflowed"))
    }
}

#[cfg(test)]
mod tests;
