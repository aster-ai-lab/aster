//! Stable noaux_tc selection and routed SwiGLU tensor composition.
//!
//! Selection consumes actual FP32 sqrt-softplus scores. `router` composes the
//! BF16-parameter/FP32-output projection and scoring control path. Fused GPU
//! routing and expert GEMMs still require separate qualification.
//! Semantics follow the pinned MIT TileKernels references.

use aster_power::error::Result;
use candle_core::{DType, Tensor};

use super::{invalid, tensor_error, CudaLayerDevice};

const MAX_EXPERTS: usize = 384;
const MAX_TOPK: usize = 6;

pub mod block;
pub mod router;

pub struct MoeRouteSelection {
    device: CudaLayerDevice,
    text_bias: Tensor,
    image_bias: Tensor,
    topk: usize,
    scale: f64,
    experts: usize,
}

impl MoeRouteSelection {
    pub fn new(
        device: CudaLayerDevice,
        text_bias: Tensor,
        image_bias: Tensor,
        topk: usize,
        scale: f64,
    ) -> Result<Self> {
        let experts = text_bias.dims1().map_err(tensor_error)?;
        validate_policy(experts, topk, scale)?;
        validate_biases(
            &text_bias,
            &image_bias,
            experts,
            device.runtime.tensor_device(),
        )?;
        device
            .limits
            .checked_state_bytes((experts * 48) as u64, "MoE correction parameters")?;
        let text_bias = text_bias.force_contiguous().map_err(tensor_error)?.detach();
        let image_bias = image_bias
            .force_contiguous()
            .map_err(tensor_error)?
            .detach();
        Ok(Self {
            device,
            text_bias,
            image_bias,
            topk,
            scale,
            experts,
        })
    }

    /// Correction biases influence selection only. Selected unbiased scores
    /// are normalized using the model's epsilon and routed scaling factor.
    /// The unfused control path validates finite values explicitly; its scalar
    /// status reads are not presented as an optimized asynchronous kernel.
    pub fn select_scores(&self, scores: &Tensor, image_mask: Option<&Tensor>) -> Result<MoeRoutes> {
        let (tokens, experts) = scores.dims2().map_err(tensor_error)?;
        if tokens == 0
            || experts != self.experts
            || !scores
                .device()
                .same_device(self.device.runtime.tensor_device())
        {
            return Err(invalid("MoE scores differ from their bound CUDA router"));
        }
        self.device
            .limits
            .checked_elements(&[tokens, experts], "MoE scores")?;
        let peak = (tokens as u64)
            .checked_mul((experts * 160 + self.topk * 32) as u64)
            .and_then(|n| n.checked_add((experts * 48) as u64))
            .ok_or_else(|| invalid("MoE selection tensor budget overflowed"))?;
        self.device
            .limits
            .checked_state_bytes(peak, "MoE selection live tensors")?;
        select_math(
            scores,
            &self.text_bias,
            &self.image_bias,
            image_mask,
            self.topk,
            self.scale,
        )
    }
}

pub struct MoeRoutes {
    logical_experts: Tensor,
    weights: Tensor,
}

impl MoeRoutes {
    pub fn logical_experts(&self) -> &Tensor {
        &self.logical_experts
    }
    pub fn weights(&self) -> &Tensor {
        &self.weights
    }
}

/// Produces FP32 values so the subsequent explicitly selected BF16/FP8 cast
/// sees the route-weighted activation. Moving route weights after a quantized
/// down projection can change rounding and is not silently substituted.
pub struct RoutedSwiGlu {
    device: CudaLayerDevice,
    limit: f64,
}

impl RoutedSwiGlu {
    pub fn new(device: CudaLayerDevice, limit: f64) -> Result<Self> {
        validate_scalar(limit)?;
        Ok(Self { device, limit })
    }

    pub fn forward(
        &self,
        gate: &Tensor,
        value: &Tensor,
        route_weights: Option<&Tensor>,
    ) -> Result<Tensor> {
        if !gate
            .device()
            .same_device(self.device.runtime.tensor_device())
        {
            return Err(invalid("SwiGLU input differs from the bound CUDA device"));
        }
        self.device
            .limits
            .checked_elements(gate.dims(), "SwiGLU activation")?;
        let peak = (gate.elem_count() as u64)
            .checked_mul(64)
            .ok_or_else(|| invalid("SwiGLU tensor-byte estimate overflowed"))?;
        self.device
            .limits
            .checked_state_bytes(peak, "SwiGLU live tensors")?;
        swiglu_math(gate, value, route_weights, self.limit)
    }
}

fn validate_scalar(value: f64) -> Result<()> {
    if !value.is_finite() || (value as f32) <= 0.0 || value > f32::MAX as f64 {
        return Err(invalid(
            "MoE scale/clip requires a positive finite FP32 value",
        ));
    }
    Ok(())
}

fn validate_policy(experts: usize, topk: usize, scale: f64) -> Result<()> {
    validate_scalar(scale)?;
    if experts == 0 || experts > MAX_EXPERTS || topk == 0 || topk > experts || topk > MAX_TOPK {
        return Err(invalid(
            "MoE routing requires 1..=384 experts and 1..=min(6,experts) selections",
        ));
    }
    Ok(())
}

fn validate_biases(
    text: &Tensor,
    image: &Tensor,
    experts: usize,
    device: &candle_core::Device,
) -> Result<()> {
    for bias in [text, image] {
        if bias.dtype() != DType::F32
            || bias.dims() != [experts]
            || !bias.device().same_device(device)
        {
            return Err(invalid(
                "MoE text/image biases require same-device FP32 expert vectors",
            ));
        }
        validate_finite(bias, false)?;
    }
    Ok(())
}

/// Read a bounded scalar status, never the logits, scores or expert weights.
/// A later fused kernel may supply the equivalent validation status directly.
fn validate_finite(tensor: &Tensor, nonnegative: bool) -> Result<()> {
    super::validate_finite(tensor, nonnegative, "MoE tensor")
}

// CPU invocation is private to tests; exported selectors require the CUDA
// device boundary. All selection arithmetic and IDs stay on their input device.
fn select_math(
    scores: &Tensor,
    text_bias: &Tensor,
    image_bias: &Tensor,
    image_mask: Option<&Tensor>,
    topk: usize,
    scale: f64,
) -> Result<MoeRoutes> {
    let (tokens, experts) = scores.dims2().map_err(tensor_error)?;
    validate_policy(experts, topk, scale)?;
    if tokens == 0 || scores.dtype() != DType::F32 {
        return Err(invalid(
            "MoE scores require nonempty FP32 [tokens,experts] data",
        ));
    }
    validate_biases(text_bias, image_bias, experts, scores.device())?;
    validate_finite(scores, true)?;
    if image_mask.is_some_and(|mask| {
        mask.dims() != [tokens]
            || mask.dtype() != DType::U8
            || !mask.device().same_device(scores.device())
    }) {
        return Err(invalid(
            "MoE image mask requires same-device U8 [tokens] data",
        ));
    }
    if let Some(mask) = image_mask {
        if mask
            .max_all()
            .and_then(|value| value.to_scalar::<u8>())
            .map_err(tensor_error)?
            > 1
        {
            return Err(invalid(
                "MoE image mask contains a value outside the boolean set 0,1",
            ));
        }
    }
    let correction = match image_mask {
        Some(mask) => mask
            .reshape((tokens, 1))
            .and_then(|mask| mask.broadcast_as((tokens, experts)))
            .and_then(|mask| {
                mask.where_cond(
                    &image_bias.broadcast_as((tokens, experts))?,
                    &text_bias.broadcast_as((tokens, experts))?,
                )
            })
            .map_err(tensor_error)?,
        None => text_bias.clone(),
    };
    let rankings = scores.broadcast_add(&correction).map_err(tensor_error)?;
    validate_finite(&rankings, false)?;
    let select = || -> candle_core::Result<(Tensor, Tensor)> {
        let device = scores.device();
        // All IDs <=384 are exact FP32 integers. Taking the minimum tied ID
        // preserves global logical-ID ordering without perturbing any score.
        let ids = Tensor::arange(0_u32, experts as u32, device)?
            .to_dtype(DType::F32)?
            .unsqueeze(0)?
            .broadcast_as((tokens, experts))?;
        let sentinel =
            Tensor::new((experts + 1) as f32, device)?.broadcast_as((tokens, experts))?;
        let excluded = Tensor::new(f32::NEG_INFINITY, device)?.broadcast_as((tokens, experts))?;
        let mut rankings = rankings;
        let mut selected = Vec::with_capacity(topk);
        let mut selected_scores = Vec::with_capacity(topk);
        for _ in 0..topk {
            let ties = rankings.broadcast_eq(&rankings.max_keepdim(1)?)?;
            let choice = ties.where_cond(&ids, &sentinel)?.min_keepdim(1)?;
            let indices = choice.to_dtype(DType::U32)?;
            selected_scores.push(scores.gather(&indices, 1)?);
            selected.push(indices);
            rankings = ids
                .broadcast_eq(&choice)?
                .where_cond(&excluded, &rankings)?;
        }
        let selected_scores = Tensor::cat(&selected_scores, 1)?;
        Ok((Tensor::cat(&selected, 1)?, selected_scores))
    };
    let (logical_experts, selected_scores) = select().map_err(tensor_error)?;
    let denominator = selected_scores
        .sum_keepdim(1)
        .and_then(|sum| sum.affine(1.0, 1e-20))
        .map_err(tensor_error)?;
    validate_finite(&denominator, true)?;
    let weights = selected_scores
        .broadcast_div(&denominator)
        .and_then(|weights| weights.affine(scale, 0.0))
        .map_err(tensor_error)?;
    let routes = MoeRoutes {
        logical_experts,
        weights,
    };
    validate_finite(&routes.weights, true)?;
    Ok(routes)
}

fn swiglu_math(
    gate: &Tensor,
    value: &Tensor,
    route_weights: Option<&Tensor>,
    limit: f64,
) -> Result<Tensor> {
    validate_scalar(limit)?;
    let (rows, width) = gate.dims2().map_err(tensor_error)?;
    if rows == 0
        || width == 0
        || gate.dtype() != DType::BF16
        || value.dtype() != DType::BF16
        || value.shape() != gate.shape()
        || !value.device().same_device(gate.device())
    {
        return Err(invalid(
            "SwiGLU requires same-device, matching nonempty BF16 matrices",
        ));
    }
    if let Some(weights) = route_weights {
        if weights.dtype() != DType::F32
            || weights.dims() != [rows]
            || !weights.device().same_device(gate.device())
        {
            return Err(invalid(
                "SwiGLU route weights require same-device FP32 [rows] data",
            ));
        }
        validate_finite(weights, true)?;
    }
    validate_finite(gate, false)?;
    validate_finite(value, false)?;
    let compute = || -> candle_core::Result<Tensor> {
        let gate = gate.to_dtype(DType::F32)?.minimum(limit)?;
        let value = value.to_dtype(DType::F32)?.clamp(-limit, limit)?;
        let activation = gate.silu()?.mul(&value)?;
        match route_weights {
            Some(weights) => activation.broadcast_mul(&weights.unsqueeze(1)?),
            None => Ok(activation),
        }
    };
    let output = compute().map_err(tensor_error)?.detach();
    validate_finite(&output, false)?;
    Ok(output)
}

#[cfg(test)]
mod tests;
