//! Unfused router control, following pinned DeepInfer `v4_modules/mlp.py` and
//! TileKernels `torch/topk.py`. Parameters/inputs are BF16, but logits and
//! sqrt-softplus scores stay FP32. Casting logits to BF16 changes routing.

use aster_power::error::{PowerError, Result};
use candle_core::{DType, Tensor};
use tokio_util::sync::CancellationToken;

use super::{
    invalid, tensor_error, validate_finite, CudaLayerDevice, MoeRouteSelection, MoeRoutes,
    MAX_EXPERTS,
};

pub struct MoeRouter {
    gate_weight: Tensor,
    selection: MoeRouteSelection,
    hidden: usize,
}

impl MoeRouter {
    pub(super) fn device(&self) -> &CudaLayerDevice {
        &self.selection.device
    }
    pub(super) fn hidden_size(&self) -> usize {
        self.hidden
    }
    pub(super) fn expert_count(&self) -> usize {
        self.selection.experts
    }
    pub(super) fn top_k(&self) -> usize {
        self.selection.topk
    }
    pub(super) fn retained_parameter_bytes(&self) -> u64 {
        (self.gate_weight.elem_count() * 2
            + self.selection.text_bias.elem_count() * 4
            + self.selection.image_bias.elem_count() * 4) as u64
    }
    pub fn new(
        device: CudaLayerDevice,
        gate_weight: Tensor,
        text_bias: Tensor,
        image_bias: Tensor,
        topk: usize,
        scale: f64,
    ) -> Result<Self> {
        let (experts, hidden) = gate_weight.dims2().map_err(tensor_error)?;
        validate_weight(&gate_weight)?;
        if !gate_weight.device().same_device(device.tensor_device()) {
            return Err(invalid(
                "MoE gate weight differs from its bound CUDA device",
            ));
        }
        device
            .limits()
            .checked_elements(&[experts, hidden], "MoE gate weights")?;
        device.limits().checked_state_bytes(
            (experts * hidden * 12 + experts * 48) as u64,
            "MoE router parameters and FP32 control workspace",
        )?;
        let selection = MoeRouteSelection::new(device, text_bias, image_bias, topk, scale)?;
        if selection.experts != experts {
            return Err(invalid(
                "MoE gate and correction biases have different expert counts",
            ));
        }
        let gate_weight = gate_weight
            .force_contiguous()
            .map_err(tensor_error)?
            .detach();
        validate_finite(&gate_weight, false)?;
        Ok(Self {
            gate_weight,
            selection,
            hidden,
        })
    }

    pub fn forward(
        &self,
        hidden: &Tensor,
        image_mask: Option<&Tensor>,
        cancellation: &CancellationToken,
    ) -> Result<MoeRoutes> {
        check_cancelled(cancellation)?;
        let (tokens, width) = hidden.dims2().map_err(tensor_error)?;
        if width != self.hidden
            || tokens == 0
            || !hidden.device().same_device(self.gate_weight.device())
        {
            return Err(invalid(
                "MoE router input differs from its bound hidden geometry/device",
            ));
        }
        let limits = self.selection.device.limits();
        limits.checked_elements(hidden.dims(), "MoE gate hidden input")?;
        limits.checked_elements(&[tokens, self.selection.experts], "MoE gate scores")?;
        let peak = (tokens as u64)
            .checked_mul((width * 12 + self.selection.experts * 192) as u64)
            .and_then(|n| n.checked_add((self.gate_weight.elem_count() * 12) as u64))
            .ok_or_else(|| invalid("MoE router tensor-byte estimate overflowed"))?;
        limits.checked_state_bytes(peak, "MoE router control live tensors")?;
        let logits = project_math(hidden, &self.gate_weight)?;
        check_cancelled(cancellation)?;
        let scores = score_math(&logits)?;
        let routes = self.selection.select_scores(&scores, image_mask)?;
        check_cancelled(cancellation)?;
        Ok(routes)
    }
}

fn validate_weight(weight: &Tensor) -> Result<()> {
    let (experts, hidden) = weight.dims2().map_err(tensor_error)?;
    if weight.dtype() != DType::BF16
        || experts == 0
        || experts > MAX_EXPERTS
        || hidden == 0
        || hidden > 5120
    {
        return Err(invalid(
            "MoE gate requires BF16 [1..384 experts,1..5120 hidden] weights",
        ));
    }
    validate_finite(weight, false)
}

// The expanded FP32 tensors preserve the exact BF16 operand values and the
// FP32 output boundary. This is an unfused correctness control, not the SM90
// BF16 mega-gate kernel, and its tensor budget is not a measured GPU peak.
fn project_math(hidden: &Tensor, weight: &Tensor) -> Result<Tensor> {
    validate_weight(weight)?;
    let (tokens, width) = hidden.dims2().map_err(tensor_error)?;
    if tokens == 0
        || hidden.dtype() != DType::BF16
        || width != weight.dims()[1]
        || !hidden.device().same_device(weight.device())
    {
        return Err(invalid(
            "MoE gate input requires same-device BF16 [tokens,hidden] data",
        ));
    }
    validate_finite(hidden, false)?;
    let logits = hidden
        .to_dtype(DType::F32)
        .and_then(|hidden| hidden.matmul(&weight.to_dtype(DType::F32)?.t()?.contiguous()?))
        .map_err(tensor_error)?
        .detach();
    validate_finite(&logits, false)?;
    Ok(logits)
}

pub(crate) fn score_math(logits: &Tensor) -> Result<Tensor> {
    let (tokens, experts) = logits.dims2().map_err(tensor_error)?;
    if logits.dtype() != DType::F32 || tokens == 0 || experts == 0 || experts > MAX_EXPERTS {
        return Err(invalid(
            "sqrt-softplus requires nonempty FP32 [tokens,1..384] logits",
        ));
    }
    validate_finite(logits, false)?;
    let compute = || -> candle_core::Result<Tensor> {
        // Compensated log1p: log(1+x)*x/((1+x)-1), returning x if 1+x
        // rounds to 1. A plain log(1+exp(-abs(logit))) loses tiny scores
        // at negative logits and changes normalized route weights.
        let exponential = logits.abs()?.neg()?.exp()?;
        let sum = exponential.affine(1., 1.)?;
        let delta = sum.affine(1., -1.)?;
        let nonzero = delta.ne(0_f32)?;
        let denominator = nonzero.where_cond(&delta, &delta.ones_like()?)?;
        let correction = sum.log()?.mul(&exponential)?.div(&denominator)?;
        let log1p = nonzero.where_cond(&correction, &exponential)?;
        logits.maximum(0_f32)?.add(&log1p)?.sqrt()
    };
    let scores = compute().map_err(tensor_error)?.detach();
    validate_finite(&scores, true)?;
    Ok(scores)
}

fn check_cancelled(cancellation: &CancellationToken) -> Result<()> {
    if cancellation.is_cancelled() {
        Err(PowerError::InferenceCancelled)
    } else {
        Ok(())
    }
}

#[cfg(test)]
mod tests;
