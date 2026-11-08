//! Explicit SM90 candidate for XQ's fixed 4x4, 20-iteration Sinkhorn schedule.
//! It preserves FP32, epsilon and axis order. Promotion requires full-model A/B.

#[cfg(any(feature = "cuda", test))]
use aster_power::error::{PowerError, Result};

pub const ABI: &str = "aster-xq-sinkhorn4-f32-v1";
pub const SOURCE: &str = include_str!("sinkhorn.cu");

#[cfg(any(feature = "cuda", test))]
#[derive(Debug, Clone, Copy)]
struct Plan {
    tokens: u32,
    grid: u32,
    output_bytes: u64,
}

#[cfg(any(feature = "cuda", test))]
impl Plan {
    fn new(capability: (i32, i32), dimensions: &[usize], budget: u64) -> Result<Self> {
        if capability != (9, 0) {
            return Err(PowerError::BackendNotAvailable(
                "fused XQ Sinkhorn requires actual SM90; no fallback".into(),
            ));
        }
        if dimensions.len() != 3
            || dimensions[1..] != [4, 4]
            || !(1..=4096).contains(&dimensions[0])
        {
            return Err(PowerError::InvalidRequest(
                "fused XQ Sinkhorn requires F32[1..4096,4,4]".into(),
            ));
        }
        let tokens = dimensions[0] as u32;
        let output_bytes = u64::from(tokens) * 16 * 4;
        if output_bytes * 32 > budget {
            return Err(PowerError::InvalidRequest(
                "fused XQ Sinkhorn validation/contiguous/output exceeds live budget".into(),
            ));
        }
        Ok(Self {
            tokens,
            grid: tokens.div_ceil(4),
            output_bytes,
        })
    }
}

#[cfg(feature = "cuda")]
mod execution;
#[cfg(feature = "cuda")]
pub(crate) use execution::normalize;

#[cfg(test)]
mod tests;
