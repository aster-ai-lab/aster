//! Rust/Candle composition of mHC prenorm, mixes, Sinkhorn and residual update.
//! Math adapted from TileKernels' MIT-licensed `tile_kernels/torch/mhc.py`,
//! revision c1cf914a82e6901c679445f872051ce44ac2cc63 (Copyright 2026 DeepSeek).
//! This is an unfused control implementation, not a claimed H200 speedup.

use aster_power::error::Result;
use candle_core::{DType, Tensor};

use super::{invalid, tensor_error, validate_finite, CudaLayerDevice};

const HEADS: usize = 4;
const MIXES: usize = HEADS * (HEADS + 2);

/// Official HF fn matrices already include norm weights. Supply their FP32
/// bytes directly; do not fold norm weights into them a second time.
pub struct MhcParameters {
    pub norm_fused_projection: Tensor,
    pub scale: Tensor,
    pub base: Tensor,
}

#[derive(Clone, Copy)]
pub struct MhcNumerics {
    pub norm_epsilon: f64,
    pub pre_epsilon: f64,
    pub sinkhorn_epsilon: f64,
    pub sinkhorn_iterations: usize,
    pub post_multiplier: f64,
}

impl MhcNumerics {
    fn validate(self) -> Result<()> {
        if [
            self.norm_epsilon,
            self.pre_epsilon,
            self.sinkhorn_epsilon,
            self.post_multiplier,
        ]
        .iter()
        .any(|value| !value.is_finite() || (*value as f32) <= 0.0 || *value > f32::MAX as f64)
            || !(1..=64).contains(&self.sinkhorn_iterations)
        {
            return Err(invalid(
                "mHC numerics require positive finite FP32 values and 1..=64 Sinkhorn iterations",
            ));
        }
        Ok(())
    }
}

pub struct MhcBlock {
    device: CudaLayerDevice,
    parameters: MhcParameters,
    numerics: MhcNumerics,
    hidden: usize,
}

impl MhcBlock {
    pub(crate) fn device(&self) -> &CudaLayerDevice {
        &self.device
    }
    pub(crate) fn hidden_size(&self) -> usize {
        self.hidden
    }
    pub(crate) fn retained_parameter_bytes(&self) -> u64 {
        ((self.parameters.norm_fused_projection.elem_count()
            + self.parameters.scale.elem_count()
            + self.parameters.base.elem_count())
            * 4) as u64
    }
    pub fn new(
        device: CudaLayerDevice,
        parameters: MhcParameters,
        numerics: MhcNumerics,
    ) -> Result<Self> {
        numerics.validate()?;
        let (mixes, width) = parameters
            .norm_fused_projection
            .dims2()
            .map_err(tensor_error)?;
        if mixes != MIXES || width == 0 || !width.is_multiple_of(HEADS) {
            return Err(invalid(
                "mHC projection requires 24 rows and four positive hidden streams",
            ));
        }
        let hidden = width / HEADS;
        if hidden > 5120 {
            return Err(invalid(
                "mHC hidden width exceeds the model's 5120 features",
            ));
        }
        device
            .limits
            .checked_elements(&[mixes, width], "mHC projection")?;
        validate_parameters(&parameters, device.runtime.tensor_device(), hidden)?;
        device.limits.checked_state_bytes(
            (parameters.norm_fused_projection.elem_count() as u64 + 27) * 16,
            "mHC parameter snapshot",
        )?;
        let parameters = freeze_parameters(parameters)?;
        validate_parameters(&parameters, device.runtime.tensor_device(), hidden)?;
        Ok(Self {
            device,
            parameters,
            numerics,
            hidden,
        })
    }

    /// Keeps the immutable residual and its exact mixes together until the
    /// sublayer finishes. A caller cannot accidentally supply another residual.
    pub fn pre(&self, residual: Tensor) -> Result<MhcStep> {
        self.validate_residual(&residual)?;
        pre_math(residual, &self.parameters, self.numerics)
    }

    /// Official hc_shifted: consume the previous sublayer's pre mix, while
    /// computing this sublayer's next mix from the current residual trunk.
    pub fn pre_shifted(&self, continuation: MhcContinuation) -> Result<MhcStep> {
        self.validate_residual(&continuation.residual)?;
        pre_math_with_previous(
            continuation.residual,
            &self.parameters,
            self.numerics,
            Some(&continuation.next_pre_mix),
        )
    }

    pub fn initial_shifted(&self, embedding: &Tensor) -> Result<MhcContinuation> {
        let (tokens, hidden) = embedding.dims2().map_err(tensor_error)?;
        if hidden != self.hidden || !embedding.device().same_device(self.device.tensor_device()) {
            return Err(invalid(
                "mHC embedding differs from its bound CUDA geometry/device",
            ));
        }
        self.device
            .limits
            .checked_elements(&[tokens, HEADS, hidden], "mHC initial streams")?;
        self.check_peak(tokens)?;
        initial_math(embedding)
    }

    fn check_peak(&self, tokens: usize) -> Result<()> {
        self.live_tensor_bytes(tokens)?;
        Ok(())
    }

    pub(crate) fn live_tensor_bytes(&self, tokens: usize) -> Result<u64> {
        let bytes = (tokens as u64)
            .checked_mul((HEADS * self.hidden * 32 + MIXES * 64) as u64)
            .and_then(|bytes| {
                bytes.checked_add(
                    (self.parameters.norm_fused_projection.elem_count() as u64 + 27) * 16,
                )
            })
            .ok_or_else(|| invalid("mHC tensor-byte estimate overflowed"))?;
        self.device
            .limits
            .checked_state_bytes(bytes, "mHC live tensors")?;
        Ok(bytes)
    }

    fn validate_residual(&self, residual: &Tensor) -> Result<()> {
        let (tokens, heads, hidden) = residual.dims3().map_err(tensor_error)?;
        if tokens == 0 || heads != HEADS || hidden != self.hidden {
            return Err(invalid(
                "mHC residual shape differs from the bound model layer",
            ));
        }
        self.device
            .limits
            .checked_elements(&[tokens, heads, hidden], "mHC residual")?;
        self.device
            .limits
            .checked_elements(&[tokens, MIXES], "mHC mixes")?;
        if !residual
            .device()
            .same_device(self.device.runtime.tensor_device())
        {
            return Err(invalid("mHC residual is on a different device"));
        }
        self.check_peak(tokens)
    }
}

pub struct MhcStep {
    residual: Tensor,
    input: Tensor,
    post_mix: Tensor,
    residual_mix: Tensor,
    pre_mix: Tensor,
}

impl MhcStep {
    pub fn input(&self) -> &Tensor {
        &self.input
    }

    pub fn next_pre_mix(&self) -> &Tensor {
        &self.pre_mix
    }

    pub fn finish_shifted(self, layer_output: &Tensor) -> Result<MhcContinuation> {
        let next_pre_mix = self.pre_mix.clone();
        Ok(MhcContinuation {
            residual: self.finish(layer_output)?,
            next_pre_mix,
        })
    }

    pub fn finish(self, layer_output: &Tensor) -> Result<Tensor> {
        if layer_output.dtype() != DType::BF16
            || layer_output.shape() != self.input.shape()
            || !layer_output.device().same_device(self.residual.device())
        {
            return Err(invalid(
                "mHC layer output must match the BF16 input shape and device",
            ));
        }
        validate_finite(layer_output, false, "mHC sublayer output")?;
        let update = || -> candle_core::Result<Tensor> {
            // Rows index source streams, columns destination streams.
            let mixed = self
                .residual_mix
                .transpose(1, 2)?
                .contiguous()?
                .matmul(&self.residual.to_dtype(DType::F32)?)?;
            let branch = layer_output
                .to_dtype(DType::F32)?
                .unsqueeze(1)?
                .broadcast_mul(&self.post_mix)?;
            mixed.add(&branch)?.to_dtype(DType::BF16)
        };
        let output = update().map_err(tensor_error)?.detach();
        validate_finite(&output, false, "mHC residual update")?;
        Ok(output)
    }
}

pub struct MhcContinuation {
    residual: Tensor,
    next_pre_mix: Tensor,
}

impl MhcContinuation {
    pub fn residual(&self) -> &Tensor {
        &self.residual
    }
    pub fn next_pre_mix(&self) -> &Tensor {
        &self.next_pre_mix
    }
    /// Engram intervenes on the completed trunk before the next pre mix.
    /// Preserve the carried mix while replacing only same-shape BF16 streams.
    pub fn with_residual(self, residual: &Tensor) -> Result<Self> {
        if residual.dtype() != DType::BF16
            || residual.shape() != self.residual.shape()
            || !residual.device().same_device(self.residual.device())
        {
            return Err(invalid(
                "mHC trunk replacement must keep its BF16 shape/device",
            ));
        }
        let residual = residual.force_contiguous().map_err(tensor_error)?.detach();
        validate_finite(&residual, false, "mHC replacement trunk")?;
        Ok(Self {
            residual,
            next_pre_mix: self.next_pre_mix,
        })
    }

    /// Shifted checkpoints have no independent HC head parameters. The final
    /// sublayer's computed pre mix reduces its updated trunk before LM norm.
    pub fn head_input(&self) -> Result<Tensor> {
        apply_mix(&self.residual, &self.next_pre_mix)
    }
}

fn initial_math(embedding: &Tensor) -> Result<MhcContinuation> {
    let (tokens, hidden) = embedding.dims2().map_err(tensor_error)?;
    if tokens == 0 || hidden == 0 || hidden > 5120 || embedding.dtype() != DType::BF16 {
        return Err(invalid(
            "mHC initial embedding requires nonempty BF16 [tokens,1..5120]",
        ));
    }
    validate_finite(embedding, false, "mHC embedding")?;
    let build = || -> candle_core::Result<MhcContinuation> {
        Ok(MhcContinuation {
            residual: embedding
                .detach()
                .unsqueeze(1)?
                .broadcast_as((tokens, HEADS, hidden))?
                .force_contiguous()?
                .detach(),
            next_pre_mix: Tensor::new(&[1_f32, 0., 0., 0.], embedding.device())?
                .reshape((1, HEADS, 1))?
                .broadcast_as((tokens, HEADS, 1))?
                .force_contiguous()?
                .detach(),
        })
    };
    build().map_err(tensor_error)
}

fn apply_mix(residual: &Tensor, mix: &Tensor) -> Result<Tensor> {
    let (tokens, heads, _) = residual.dims3().map_err(tensor_error)?;
    if residual.dtype() != DType::BF16
        || heads != HEADS
        || mix.dtype() != DType::F32
        || mix.dims() != [tokens, HEADS, 1]
        || !mix.device().same_device(residual.device())
    {
        return Err(invalid(
            "mHC prior mix requires same-device FP32 [tokens,4,1] for a BF16 trunk",
        ));
    }
    validate_finite(mix, true, "mHC prior pre mix")?;
    let output = residual
        .to_dtype(DType::F32)
        .and_then(|value| value.broadcast_mul(mix))
        .and_then(|value| value.sum(1))
        .and_then(|value| value.to_dtype(DType::BF16))
        .map_err(tensor_error)?
        .detach();
    validate_finite(&output, false, "mHC mixed input")?;
    Ok(output)
}

fn freeze_parameters(parameters: MhcParameters) -> Result<MhcParameters> {
    Ok(MhcParameters {
        norm_fused_projection: parameters
            .norm_fused_projection
            .force_contiguous()
            .map_err(tensor_error)?
            .detach(),
        scale: parameters
            .scale
            .force_contiguous()
            .map_err(tensor_error)?
            .detach(),
        base: parameters
            .base
            .force_contiguous()
            .map_err(tensor_error)?
            .detach(),
    })
}

fn validate_parameters(
    parameters: &MhcParameters,
    device: &candle_core::Device,
    hidden: usize,
) -> Result<()> {
    for (tensor, shape) in [
        (
            &parameters.norm_fused_projection,
            vec![MIXES, HEADS * hidden],
        ),
        (&parameters.scale, vec![3]),
        (&parameters.base, vec![MIXES]),
    ] {
        if tensor.dtype() != DType::F32
            || tensor.dims() != shape
            || !tensor.device().same_device(device)
        {
            return Err(invalid(
                "mHC requires same-device FP32 projection [24,4*hidden], scale [3], base [24]",
            ));
        }
        validate_finite(tensor, false, "mHC effective parameter")?;
    }
    Ok(())
}

// CPU invocation is private to hermetic tests. Public MhcBlock construction
// requires CudaLayerDevice; production does not select CPU if CUDA fails.
fn pre_math(
    residual: Tensor,
    parameters: &MhcParameters,
    numerics: MhcNumerics,
) -> Result<MhcStep> {
    pre_math_with_previous(residual, parameters, numerics, None)
}

fn pre_math_with_previous(
    residual: Tensor,
    parameters: &MhcParameters,
    numerics: MhcNumerics,
    previous: Option<&Tensor>,
) -> Result<MhcStep> {
    numerics.validate()?;
    let (tokens, heads, hidden) = residual.dims3().map_err(tensor_error)?;
    if residual.dtype() != DType::BF16 || tokens == 0 || heads != HEADS || hidden == 0 {
        return Err(invalid(
            "mHC residual requires nonempty [tokens,4,hidden] BF16 data",
        ));
    }
    validate_parameters(parameters, residual.device(), hidden)?;
    validate_finite(&residual, false, "mHC residual")?;
    let residual = residual.force_contiguous().map_err(tensor_error)?.detach();
    if let Some(previous) = previous {
        if previous.dtype() != DType::F32
            || previous.dims() != [tokens, HEADS, 1]
            || !previous.device().same_device(residual.device())
        {
            return Err(invalid("mHC prior mix differs from this trunk"));
        }
        validate_finite(previous, true, "mHC prior pre mix")?;
    }
    let compute = || -> candle_core::Result<MhcStep> {
        let x = residual.to_dtype(DType::F32)?;
        let flat = x.flatten_from(1)?;
        let denominator = flat
            .sqr()?
            .mean_keepdim(1)?
            .affine(1.0, numerics.norm_epsilon)?
            .sqrt()?;
        validate_finite(&denominator, false, "mHC RMS denominator")
            .map_err(|error| candle_core::Error::Msg(error.to_string()))?;
        let mixes = flat
            .matmul(&parameters.norm_fused_projection.t()?)?
            .broadcast_div(&denominator)?;
        validate_finite(&mixes, false, "mHC projected mixes")
            .map_err(|error| candle_core::Error::Msg(error.to_string()))?;
        let group = |offset: usize, width: usize, scale: usize| -> candle_core::Result<Tensor> {
            let group = mixes
                .narrow(1, offset, width)?
                .broadcast_mul(&parameters.scale.narrow(0, scale, 1)?)?
                .broadcast_add(&parameters.base.narrow(0, offset, width)?)?;
            validate_finite(&group, false, "mHC scaled mix logits")
                .map_err(|error| candle_core::Error::Msg(error.to_string()))?;
            Ok(group)
        };
        let pre_mix = candle_nn::ops::sigmoid(&group(0, HEADS, 0)?)?
            .affine(1.0, numerics.pre_epsilon)?
            .unsqueeze(2)?;
        let post_mix = candle_nn::ops::sigmoid(&group(HEADS, HEADS, 1)?)?
            .affine(numerics.post_multiplier, 0.0)?
            .unsqueeze(2)?;
        let raw = group(2 * HEADS, HEADS * HEADS, 2)?.reshape((tokens, HEADS, HEADS))?;
        let mut residual_mix =
            candle_nn::ops::softmax_last_dim(&raw)?.affine(1.0, numerics.sinkhorn_epsilon)?;
        residual_mix = normalize_axis(&residual_mix, 1, numerics.sinkhorn_epsilon)?;
        for _ in 1..numerics.sinkhorn_iterations {
            residual_mix = normalize_axis(&residual_mix, 2, numerics.sinkhorn_epsilon)?;
            residual_mix = normalize_axis(&residual_mix, 1, numerics.sinkhorn_epsilon)?;
        }
        let input_mix = previous.unwrap_or(&pre_mix);
        let input = x.broadcast_mul(input_mix)?.sum(1)?.to_dtype(DType::BF16)?;
        Ok(MhcStep {
            residual: residual.clone(),
            input,
            post_mix,
            residual_mix,
            pre_mix,
        })
    };
    let output = compute().map_err(tensor_error)?;
    for (tensor, label) in [
        (&output.input, "mHC input"),
        (&output.pre_mix, "mHC next pre mix"),
        (&output.post_mix, "mHC post mix"),
        (&output.residual_mix, "mHC residual mix"),
    ] {
        validate_finite(tensor, false, label)?;
    }
    Ok(output)
}

fn normalize_axis(tensor: &Tensor, axis: usize, epsilon: f64) -> candle_core::Result<Tensor> {
    let denominator = tensor.sum_keepdim(axis)?.affine(1.0, epsilon)?;
    validate_finite(&denominator, true, "mHC Sinkhorn denominator")
        .map_err(|error| candle_core::Error::Msg(error.to_string()))?;
    tensor.broadcast_div(&denominator)
}

#[cfg(test)]
mod tests;
