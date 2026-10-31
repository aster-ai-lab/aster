//! Unfused rotary transform over an explicit FP32 [cos | sin] cache.
//!
//! The model-specific factory constructs the official FP32 base/YaRN caches;
//! actual CUDA numerical qualification remains a separate requirement. The
//! explicit-cache entry preserves supplied values, including amplitude scaling.
//! Semantics follow DeepInfer's Apache-2.0 rotary contract at revision
//! 9bd9cf243fc53ef9afbf8f04d391d989117d6ab9. Original notices are retained
//! in licenses/Rotary-Apache-NOTICE.txt.

use aster_power::error::{PowerError, Result};
use aster_power::inference::InferenceLimits;
use candle_core::{DType, Tensor};
use tokio_util::sync::CancellationToken;

use super::{invalid, tensor_error, validate_finite, CudaLayerDevice};
use crate::model::MAX_CONTEXT_TOKENS;

pub mod factory;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RopeLayout {
    Interleaved,
    HalfSplit,
}

impl RopeLayout {
    pub fn parse(value: &str) -> Result<Self> {
        match value {
            "interleaved" => Ok(Self::Interleaved),
            "half_split" => Ok(Self::HalfSplit),
            _ => Err(invalid(
                "unsupported rotary layout; supported set: interleaved, half_split",
            )),
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Interleaved => "interleaved",
            Self::HalfSplit => "half_split",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RopeDirection {
    Forward,
    Conjugate,
}

impl RopeDirection {
    pub fn parse(value: &str) -> Result<Self> {
        match value {
            "forward" => Ok(Self::Forward),
            "conjugate" => Ok(Self::Conjugate),
            _ => Err(invalid(
                "unsupported rotary direction; supported set: forward, conjugate",
            )),
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Forward => "forward",
            Self::Conjugate => "conjugate",
        }
    }
}

pub struct RotaryTransform {
    device: CudaLayerDevice,
    cache: RotaryCache,
}

impl RotaryTransform {
    /// Compare finite FP32 cache values on device, returning one status scalar.
    /// This guards composition; it does not establish checkpoint/cache provenance.
    pub(crate) fn same_cache_values(&self, other: &Self) -> Result<bool> {
        if self.cache.context != other.cache.context
            || self.cache.head_dim != other.cache.head_dim
            || self.cache.rotary_dim != other.cache.rotary_dim
            || self.cache.rotary_offset != other.cache.rotary_offset
            || self.cache.layout != other.cache.layout
            || !self
                .device
                .tensor_device()
                .same_device(other.device.tensor_device())
        {
            return Ok(false);
        }
        let count = u32::try_from(self.cache.cos_sin.elem_count())
            .map_err(|_| invalid("rotary cache equality count exceeds u32"))?;
        let equal = self
            .cache
            .cos_sin
            .eq(&other.cache.cos_sin)
            .and_then(|t| t.to_dtype(DType::U32))
            .and_then(|t| t.sum_all())
            .and_then(|t| t.to_scalar::<u32>())
            .map_err(tensor_error)?;
        Ok(equal == count)
    }

    pub(crate) fn retained_cache_bytes(&self) -> u64 {
        self.cache.cos_sin.elem_count() as u64 * 4
    }

    pub(crate) fn context_tokens(&self) -> usize {
        self.cache.context
    }

    /// Bind an already qualified, same-device FP32 [context, rotary_dim] cache.
    /// The transformed interval is explicit; other feature bits are preserved.
    pub fn from_cache(
        device: CudaLayerDevice,
        head_dim: usize,
        rotary_offset: usize,
        layout: RopeLayout,
        cos_sin: Tensor,
    ) -> Result<Self> {
        if !cos_sin.device().same_device(device.tensor_device()) {
            return Err(invalid("rotary cache differs from its bound CUDA device"));
        }
        let cache = RotaryCache::new(device.limits(), head_dim, rotary_offset, layout, &cos_sin)?;
        Ok(Self { device, cache })
    }

    pub fn forward(
        &self,
        input: &Tensor,
        positions: &[usize],
        direction: RopeDirection,
        cancellation: &CancellationToken,
    ) -> Result<Tensor> {
        self.cache.forward(
            self.device.limits(),
            input,
            positions,
            direction,
            cancellation,
        )
    }

    /// Explicit F32 activation profile for XQ. Cache arithmetic and output
    /// remain F32, including the unrotated interval; the existing BF16 entry
    /// retains its original input/output boundary.
    pub fn forward_f32_control(
        &self,
        input: &Tensor,
        positions: &[usize],
        direction: RopeDirection,
        cancellation: &CancellationToken,
    ) -> Result<Tensor> {
        self.cache.forward_typed(
            self.device.limits(),
            input,
            positions,
            direction,
            DType::F32,
            cancellation,
        )
    }
}

// Private CPU access exists solely for sibling numerical tests, never serving.
struct RotaryCache {
    cos_sin: Tensor,
    context: usize,
    head_dim: usize,
    rotary_dim: usize,
    rotary_offset: usize,
    layout: RopeLayout,
}

impl RotaryCache {
    fn new(
        limits: &InferenceLimits,
        head_dim: usize,
        rotary_offset: usize,
        layout: RopeLayout,
        cache: &Tensor,
    ) -> Result<Self> {
        limits.validate()?;
        let (context, rotary_dim) = cache.dims2().map_err(tensor_error)?;
        if cache.dtype() != DType::F32
            || context == 0
            || context > limits.max_context_tokens
            || context > MAX_CONTEXT_TOKENS
            || rotary_dim == 0
            || rotary_dim > 64
            || rotary_dim % 2 != 0
            || head_dim == 0
            || head_dim > 512
            || rotary_offset
                .checked_add(rotary_dim)
                .is_none_or(|end| end > head_dim)
        {
            return Err(invalid("rotary cache requires FP32 context rows, even width 2..=64, and a valid interval within head width 1..=512"));
        }
        limits.checked_elements(cache.dims(), "rotary cache")?;
        let bytes = (cache.elem_count() as u64)
            .checked_mul(32)
            .ok_or_else(|| invalid("rotary cache construction budget overflowed"))?;
        limits.checked_state_bytes(bytes, "rotary cache construction tensors")?;
        let cos_sin = cache.force_contiguous().map_err(tensor_error)?.detach();
        validate_finite(&cos_sin, false, "rotary cache")?;
        Ok(Self {
            cos_sin,
            context,
            head_dim,
            rotary_dim,
            rotary_offset,
            layout,
        })
    }

    fn forward(
        &self,
        limits: &InferenceLimits,
        input: &Tensor,
        positions: &[usize],
        direction: RopeDirection,
        cancellation: &CancellationToken,
    ) -> Result<Tensor> {
        self.forward_typed(
            limits,
            input,
            positions,
            direction,
            DType::BF16,
            cancellation,
        )
    }

    fn forward_typed(
        &self,
        limits: &InferenceLimits,
        input: &Tensor,
        positions: &[usize],
        direction: RopeDirection,
        dtype: DType,
        cancellation: &CancellationToken,
    ) -> Result<Tensor> {
        check_cancelled(cancellation)?;
        let (tokens, heads, width) = input.dims3().map_err(tensor_error)?;
        if !matches!(dtype, DType::BF16 | DType::F32)
            || input.dtype() != dtype
            || !input.device().same_device(self.cos_sin.device())
            || tokens == 0
            || tokens > limits.max_context_tokens
            || tokens > MAX_CONTEXT_TOKENS
            || heads == 0
            || heads > 64
            || width != self.head_dim
            || positions.len() != tokens
            || positions.iter().any(|position| *position >= self.context)
        {
            return Err(invalid("rotary input requires its declared BF16/F32 precision, same-device [tokens,1..=64,head_dim] and one in-cache position per token"));
        }
        limits.checked_elements(input.dims(), "rotary input")?;
        // Conservative named-tensor bounds, including the retained cache.
        // Model-wide Power reservation and CUDA allocator/workspace accounting
        // remain separate obligations; this is not a measured GPU peak.
        let bytes = (input.elem_count() as u64)
            .checked_mul(48)
            .and_then(|bytes| bytes.checked_add((self.cos_sin.elem_count() as u64) * 4))
            .and_then(|bytes| bytes.checked_add((tokens as u64) * (self.rotary_dim as u64) * 16))
            .and_then(|bytes| bytes.checked_add((tokens as u64) * 4))
            .ok_or_else(|| invalid("rotary live-tensor budget overflowed"))?;
        limits.checked_state_bytes(bytes, "rotary live tensors")?;
        validate_finite(input, false, "rotary input")?;
        let indices = positions
            .iter()
            .map(|position| {
                u32::try_from(*position).map_err(|_| invalid("rotary position exceeds u32"))
            })
            .collect::<Result<Vec<_>>>()?;
        let math = || -> candle_core::Result<Tensor> {
            let indices = Tensor::from_vec(indices, tokens, input.device())?;
            let cache = self.cos_sin.index_select(&indices, 0)?;
            let pairs = self.rotary_dim / 2;
            let cos = cache.narrow(1, 0, pairs)?.unsqueeze(1)?;
            let sin = cache.narrow(1, pairs, pairs)?.unsqueeze(1)?;
            // Conjugation negates sin but does not divide by the cache's
            // amplitude. With nonunit amplitude this is not a true inverse.
            let sin = match direction {
                RopeDirection::Forward => sin,
                RopeDirection::Conjugate => sin.neg()?,
            };
            let rotary = input
                .detach()
                .narrow(2, self.rotary_offset, self.rotary_dim)?
                .to_dtype(DType::F32)?
                .contiguous()?;
            let (first, second) = match self.layout {
                RopeLayout::Interleaved => {
                    let paired = rotary.reshape((tokens, heads, pairs, 2))?;
                    (
                        paired.narrow(3, 0, 1)?.squeeze(3)?,
                        paired.narrow(3, 1, 1)?.squeeze(3)?,
                    )
                }
                RopeLayout::HalfSplit => {
                    (rotary.narrow(2, 0, pairs)?, rotary.narrow(2, pairs, pairs)?)
                }
            };
            let real = first
                .broadcast_mul(&cos)?
                .sub(&second.broadcast_mul(&sin)?)?;
            let imag = second
                .broadcast_mul(&cos)?
                .add(&first.broadcast_mul(&sin)?)?;
            let rotated = match self.layout {
                RopeLayout::Interleaved => {
                    Tensor::stack(&[real, imag], 3)?.reshape((tokens, heads, self.rotary_dim))?
                }
                RopeLayout::HalfSplit => Tensor::cat(&[real, imag], 2)?,
            }
            .to_dtype(dtype)?;
            let mut parts = Vec::with_capacity(3);
            if self.rotary_offset > 0 {
                parts.push(input.narrow(2, 0, self.rotary_offset)?);
            }
            parts.push(rotated);
            let end = self.rotary_offset + self.rotary_dim;
            if end < self.head_dim {
                parts.push(input.narrow(2, end, self.head_dim - end)?);
            }
            Tensor::cat(&parts, 2)
        };
        let output = math().map_err(tensor_error)?.detach();
        validate_finite(&output, false, "rotary output")?;
        check_cancelled(cancellation)?;
        Ok(output)
    }
}

fn check_cancelled(cancellation: &CancellationToken) -> Result<()> {
    if cancellation.is_cancelled() {
        return Err(PowerError::InferenceCancelled);
    }
    Ok(())
}

#[cfg(test)]
mod tests;
