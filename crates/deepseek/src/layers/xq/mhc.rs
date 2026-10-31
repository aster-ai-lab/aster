//! XQ's F32 shifted trunk. RMS precedes the exported F16 mix projection,
//! and no BF16 cast is inserted into the residual, branch or head reduction.

use aster_power::error::Result;
use candle_core::{DType, Tensor};
use tokio_util::sync::CancellationToken;

use super::{check_cancel, invalid, rms_math, same_device};
use crate::layers::{linear::xq::XqDeviceMatrix, tensor_error, validate_finite, CudaLayerDevice};
use crate::weights::xq::{storage::XqWeights, XqTensorType};

const STREAMS: usize = 4;
const MIXES: usize = 24;
const EPSILON: f64 = 1e-6;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum XqMhcSite {
    Attention,
    FeedForward,
}

impl XqMhcSite {
    pub fn parse(value: &str) -> Result<Self> {
        match value {
            "attn" => Ok(Self::Attention),
            "ffn" => Ok(Self::FeedForward),
            _ => Err(invalid("unsupported XQ mHC site; supported set: attn, ffn")),
        }
    }
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Attention => "attn",
            Self::FeedForward => "ffn",
        }
    }
}

pub struct XqMhcBlock {
    device: CudaLayerDevice,
    projection: XqDeviceMatrix,
    scale: Tensor,
    base: Tensor,
    hidden: usize,
}

pub struct XqMhcContinuation {
    residual: Tensor,
    next_pre_mix: Tensor,
}

pub struct XqMhcStep {
    residual: Tensor,
    input: Tensor,
    pre_mix: Tensor,
    post_mix: Tensor,
    residual_mix: Tensor,
}

impl XqMhcBlock {
    pub fn load(
        device: &CudaLayerDevice,
        weights: &XqWeights,
        layer: usize,
        site: XqMhcSite,
        cancellation: &CancellationToken,
    ) -> Result<Self> {
        if layer >= 40 {
            return Err(invalid("XQ mHC layer requires a backbone index 0..39"));
        }
        let prefix = format!("blk.{layer}.hc_{}", site.as_str());
        let name = format!("{prefix}_fn.weight");
        let descriptor = weights.tensor(&name)?;
        if descriptor.kind != XqTensorType::F16 || descriptor.shape != [20480, MIXES as u64] {
            return Err(invalid(
                "XQ mHC projection differs from exact F16[20480,24] storage",
            ));
        }
        let projection = XqDeviceMatrix::load(device, weights, &name, 0, MIXES, cancellation)?;
        let scale = weights.load_f32_parameter(
            device,
            &format!("{prefix}_scale.weight"),
            XqTensorType::F32,
            &[3],
            cancellation,
        )?;
        let base = weights.load_f32_parameter(
            device,
            &format!("{prefix}_base.weight"),
            XqTensorType::F32,
            &[24],
            cancellation,
        )?;
        Self::new(device.clone(), projection, scale, base)
    }

    pub fn new(
        device: CudaLayerDevice,
        projection: XqDeviceMatrix,
        scale: Tensor,
        base: Tensor,
    ) -> Result<Self> {
        let [mixes, width] = projection.logical_shape();
        if mixes != MIXES
            || width == 0
            || !width.is_multiple_of(STREAMS)
            || width / STREAMS > 5120
            || projection.kind() != XqTensorType::F16
        {
            return Err(invalid(
                "XQ mHC requires a packed F16 projection [24,4*hidden], hidden 1..5120",
            ));
        }
        if !device
            .tensor_device()
            .same_device(projection.device().tensor_device())
            || device.limits() != projection.device().limits()
        {
            return Err(invalid("XQ mHC packed projection differs from its device"));
        }
        if projection.retained_weight_bytes() + 108 > device.limits().max_resident_weight_bytes {
            return Err(invalid("XQ mHC parameters exceed their resident budget"));
        }
        device
            .limits()
            .checked_state_bytes(108 * 16, "XQ mHC scale/base snapshot and validation")?;
        validate_parameters(&scale, &base, device.tensor_device())?;
        let scale = scale.force_contiguous().map_err(tensor_error)?.detach();
        let base = base.force_contiguous().map_err(tensor_error)?.detach();
        Ok(Self {
            device,
            projection,
            scale,
            base,
            hidden: width / STREAMS,
        })
    }

    pub fn hidden_size(&self) -> usize {
        self.hidden
    }
    pub(crate) fn device(&self) -> &CudaLayerDevice {
        &self.device
    }
    pub(crate) fn live_tensor_bytes(&self, tokens: usize) -> Result<u64> {
        live_bytes(tokens, self.hidden)
    }
    pub fn retained_bytes(&self) -> u64 {
        self.projection.retained_weight_bytes() + 108
    }

    pub fn prepare(
        &self,
        continuation: XqMhcContinuation,
        cancellation: &CancellationToken,
    ) -> Result<XqMhcStep> {
        check_cancel(cancellation)?;
        let tokens = validate_continuation(&continuation, self.hidden, &self.device)?;
        let live = live_bytes(tokens, self.hidden)?
            .checked_add(self.retained_bytes())
            .ok_or_else(|| invalid("XQ mHC retained/live byte bound overflowed"))?;
        self.device
            .limits()
            .checked_state_bytes(live, "XQ shifted mHC prepare/finish tensors")?;
        // Pinned GGUF execution applies RMS to flat F32 values before the
        // F16/F32 dot, unlike the official checkpoint's project/divide order.
        let flat = continuation
            .residual
            .flatten_from(1)
            .map_err(tensor_error)?;
        let normalized = rms_math(&flat, None)?;
        let (projected, _) = self
            .projection
            .forward_f32_control(&normalized, cancellation)?;
        let output = finish_prepare(continuation, &projected, &self.scale, &self.base)?;
        check_cancel(cancellation)?;
        Ok(output)
    }
}

impl XqMhcContinuation {
    pub fn from_embedding(
        device: &CudaLayerDevice,
        embedding: &Tensor,
        cancellation: &CancellationToken,
    ) -> Result<Self> {
        check_cancel(cancellation)?;
        same_device(device, embedding)?;
        let (tokens, hidden) = embedding.dims2().map_err(tensor_error)?;
        if tokens == 0 || hidden == 0 || hidden > 5120 || embedding.dtype() != DType::F32 {
            return Err(invalid("XQ mHC embedding requires F32[tokens,1..5120]"));
        }
        device
            .limits()
            .checked_elements(&[tokens, STREAMS, hidden], "XQ mHC initial trunk")?;
        device
            .limits()
            .checked_state_bytes(live_bytes(tokens, hidden)?, "XQ mHC initial F32 streams")?;
        validate_finite(embedding, false, "XQ mHC embedding")?;
        let residual = embedding
            .unsqueeze(1)
            .and_then(|x| x.broadcast_as((tokens, STREAMS, hidden)))
            .and_then(|x| x.force_contiguous())
            .map_err(tensor_error)?
            .detach();
        let next_pre_mix = Tensor::new(&[1_f32, 0., 0., 0.], embedding.device())
            .and_then(|x| x.reshape((1, STREAMS, 1)))
            .and_then(|x| x.broadcast_as((tokens, STREAMS, 1)))
            .and_then(|x| x.force_contiguous())
            .map_err(tensor_error)?
            .detach();
        check_cancel(cancellation)?;
        Ok(Self {
            residual,
            next_pre_mix,
        })
    }

    pub fn residual(&self) -> &Tensor {
        &self.residual
    }

    pub(crate) fn move_to(
        self,
        target: &CudaLayerDevice,
        transfers: &mut crate::parallel::XqStepTransfers<'_>,
        cancellation: &CancellationToken,
    ) -> Result<Self> {
        check_cancel(cancellation)?;
        let residual = transfers.copy(&self.residual, target, cancellation)?;
        let next_pre_mix = transfers.copy(&self.next_pre_mix, target, cancellation)?;
        Ok(Self {
            residual,
            next_pre_mix,
        })
    }
    pub fn next_pre_mix(&self) -> &Tensor {
        &self.next_pre_mix
    }

    pub fn with_residual(self, residual: &Tensor) -> Result<Self> {
        if residual.dtype() != DType::F32
            || residual.shape() != self.residual.shape()
            || !residual.device().same_device(self.residual.device())
        {
            return Err(invalid(
                "XQ trunk replacement must preserve F32 shape/device",
            ));
        }
        let residual = residual.force_contiguous().map_err(tensor_error)?.detach();
        validate_finite(&residual, false, "XQ replacement trunk")?;
        Ok(Self {
            residual,
            next_pre_mix: self.next_pre_mix,
        })
    }

    pub fn head_input(&self) -> Result<Tensor> {
        apply_mix(&self.residual, &self.next_pre_mix)
    }
}

impl XqMhcStep {
    pub fn input(&self) -> &Tensor {
        &self.input
    }
    pub fn next_pre_mix(&self) -> &Tensor {
        &self.pre_mix
    }

    pub fn finish(
        self,
        branch: &Tensor,
        cancellation: &CancellationToken,
    ) -> Result<XqMhcContinuation> {
        check_cancel(cancellation)?;
        if branch.dtype() != DType::F32
            || branch.shape() != self.input.shape()
            || !branch.device().same_device(self.input.device())
        {
            return Err(invalid(
                "XQ mHC branch must match its F32 input shape/device",
            ));
        }
        validate_finite(branch, false, "XQ mHC branch")?;
        crate::layers::attention::check_f32_matmul_precision(DType::F32, self.residual.device())?;
        let mixed = self
            .residual_mix
            .transpose(1, 2)
            .and_then(|x| x.contiguous())
            .and_then(|x| x.matmul(&self.residual))
            .map_err(tensor_error)?;
        let update = branch
            .unsqueeze(1)
            .and_then(|x| x.broadcast_mul(&self.post_mix))
            .map_err(tensor_error)?;
        let residual = mixed.add(&update).map_err(tensor_error)?.detach();
        validate_finite(&residual, false, "XQ mHC residual update")?;
        check_cancel(cancellation)?;
        Ok(XqMhcContinuation {
            residual,
            next_pre_mix: self.pre_mix,
        })
    }
}

fn finish_prepare(
    continuation: XqMhcContinuation,
    projected: &Tensor,
    scale: &Tensor,
    base: &Tensor,
) -> Result<XqMhcStep> {
    let (tokens, heads, hidden) = continuation.residual.dims3().map_err(tensor_error)?;
    if tokens == 0
        || heads != STREAMS
        || hidden == 0
        || continuation.residual.dtype() != DType::F32
        || projected.dtype() != DType::F32
        || projected.dims() != [tokens, MIXES]
        || !projected
            .device()
            .same_device(continuation.residual.device())
    {
        return Err(invalid(
            "XQ projected mHC mixes differ from their F32 trunk/24-mix geometry",
        ));
    }
    validate_parameters(scale, base, projected.device())?;
    validate_finite(projected, false, "XQ projected mHC mixes")?;
    let group = |offset, width, index| -> Result<Tensor> {
        let value = projected
            .narrow(1, offset, width)
            .and_then(|x| x.broadcast_mul(&scale.narrow(0, index, 1)?))
            .and_then(|x| x.broadcast_add(&base.narrow(0, offset, width)?))
            .map_err(tensor_error)?;
        validate_finite(&value, false, "XQ mHC mix logits")?;
        Ok(value)
    };
    let pre_mix = candle_nn::ops::sigmoid(&group(0, 4, 0)?)
        .and_then(|x| x.affine(1., EPSILON))
        .and_then(|x| x.unsqueeze(2))
        .map_err(tensor_error)?;
    let post_mix = candle_nn::ops::sigmoid(&group(4, 4, 1)?)
        .and_then(|x| x.affine(2., 0.))
        .and_then(|x| x.unsqueeze(2))
        .map_err(tensor_error)?;
    let raw = group(8, 16, 2)?
        .reshape((tokens, 4, 4))
        .map_err(tensor_error)?;
    let residual_mix = candle_nn::ops::softmax_last_dim(&raw)
        .and_then(|x| x.affine(1., EPSILON))
        .map_err(tensor_error)?;
    let residual_mix = sinkhorn(&residual_mix)?;
    let input = apply_mix(&continuation.residual, &continuation.next_pre_mix)?;
    for value in [&pre_mix, &post_mix, &residual_mix] {
        validate_finite(value, true, "XQ mHC mixes")?;
    }
    Ok(XqMhcStep {
        residual: continuation.residual,
        input,
        pre_mix,
        post_mix,
        residual_mix,
    })
}

fn apply_mix(residual: &Tensor, mix: &Tensor) -> Result<Tensor> {
    let (tokens, heads, _) = residual.dims3().map_err(tensor_error)?;
    if tokens == 0
        || heads != STREAMS
        || residual.dtype() != DType::F32
        || mix.dtype() != DType::F32
        || mix.dims() != [tokens, STREAMS, 1]
        || !mix.device().same_device(residual.device())
    {
        return Err(invalid(
            "XQ shifted mix requires F32 trunk and nonnegative F32[tokens,4,1] mix",
        ));
    }
    validate_finite(residual, false, "XQ shifted trunk")?;
    validate_finite(mix, true, "XQ prior pre mix")?;
    let output = residual
        .broadcast_mul(mix)
        .and_then(|x| x.sum(1))
        .map_err(tensor_error)?
        .detach();
    validate_finite(&output, false, "XQ mixed input")?;
    Ok(output)
}

fn validate_parameters(scale: &Tensor, base: &Tensor, device: &candle_core::Device) -> Result<()> {
    for (value, size) in [(scale, 3), (base, 24)] {
        if value.dtype() != DType::F32
            || value.dims() != [size]
            || !value.device().same_device(device)
        {
            return Err(invalid(
                "XQ mHC requires F32 scale[3] and base[24] on its device",
            ));
        }
        validate_finite(value, false, "XQ mHC scale/base")?;
    }
    Ok(())
}

fn sinkhorn(tensor: &Tensor) -> Result<Tensor> {
    #[cfg(feature = "xq-fused-sinkhorn")]
    if tensor.device().is_cuda() {
        return crate::cuda::sinkhorn::normalize(tensor, 16 * 1024 * 1024);
    }
    #[cfg(all(feature = "xq-fused-sinkhorn", not(test)))]
    return Err(invalid("fused XQ Sinkhorn profile requires CUDA"));
    #[cfg(any(not(feature = "xq-fused-sinkhorn"), test))]
    tensor_sinkhorn(tensor)
}

#[cfg(any(not(feature = "xq-fused-sinkhorn"), test))]
pub(crate) fn tensor_sinkhorn(tensor: &Tensor) -> Result<Tensor> {
    let mut result = normalize_axis(tensor, 1)?;
    for _ in 1..20 {
        result = normalize_axis(&result, 2)?;
        result = normalize_axis(&result, 1)?;
    }
    Ok(result)
}

#[cfg(any(not(feature = "xq-fused-sinkhorn"), test))]
fn normalize_axis(tensor: &Tensor, axis: usize) -> Result<Tensor> {
    let denominator = tensor
        .sum_keepdim(axis)
        .and_then(|x| x.affine(1., EPSILON))
        .map_err(tensor_error)?;
    validate_finite(&denominator, true, "XQ Sinkhorn denominator")?;
    tensor.broadcast_div(&denominator).map_err(tensor_error)
}

fn validate_continuation(
    value: &XqMhcContinuation,
    hidden: usize,
    device: &CudaLayerDevice,
) -> Result<usize> {
    let (tokens, heads, width) = value.residual.dims3().map_err(tensor_error)?;
    if tokens == 0 || heads != STREAMS || width != hidden || value.residual.dtype() != DType::F32 {
        return Err(invalid(
            "XQ continuation differs from its block's F32 shape",
        ));
    }
    same_device(device, &value.residual)?;
    device
        .limits()
        .checked_elements(&[tokens, heads, width], "XQ mHC trunk")?;
    if value.next_pre_mix.dtype() != DType::F32 || value.next_pre_mix.dims() != [tokens, STREAMS, 1]
    {
        return Err(invalid(
            "XQ continuation prior mix has invalid geometry/type",
        ));
    }
    same_device(device, &value.next_pre_mix)?;
    Ok(tokens)
}

fn live_bytes(tokens: usize, hidden: usize) -> Result<u64> {
    (tokens as u64)
        .checked_mul((STREAMS * hidden * 80 + MIXES * 256) as u64)
        .ok_or_else(|| invalid("XQ mHC live tensor-byte bound overflowed"))
}

#[cfg(test)]
mod tests;
