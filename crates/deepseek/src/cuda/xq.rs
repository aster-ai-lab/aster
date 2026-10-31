//! Explicit packed XQ F32 projection control. GPU execution is gated
//! independently from CPU layout validation and whole-model activation.

#[cfg(any(feature = "cuda", test))]
use aster_power::error::{PowerError, Result};
use sha2::{Digest, Sha256};

use super::KernelAttributes;
#[cfg(any(feature = "cuda", test))]
use crate::weights::xq::XqTensorType;

pub const XQ_CUDA_ABI: &str = "aster-sm90-xq-warp-f32-v1";
pub const XQ_GATHER_ABI: &str = "aster-sm90-xq-gather-f32-v1";
pub const XQ_CUDA_SOURCE: &str = include_str!("xq.cu");
pub const XQ_CUDA_OPTIONS: &str = "compute_90;ftz=false;fmad=false;fast_math=false";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct XqProjectionReceipt {
    pub abi: &'static str,
    pub source_sha256: String,
    pub ptx_sha256: String,
    pub compile_options: &'static str,
    pub kernel_attributes: KernelAttributes,
    pub output_bytes: u64,
    pub scratch_bytes: u64,
}

pub fn source_sha256() -> String {
    format!("{:x}", Sha256::digest(XQ_CUDA_SOURCE.as_bytes()))
}

#[cfg(any(feature = "cuda", test))]
#[derive(Clone, Copy, Debug)]
pub(crate) struct XqPlan {
    pub tokens: u32,
    pub rows: u32,
    pub columns: u32,
    pub type_id: u32,
    pub row_bytes: u32,
    pub grid_x: u32,
    pub output_bytes: u64,
}

#[cfg(any(feature = "cuda", test))]
impl XqPlan {
    pub(crate) fn new(
        capability: (i32, i32),
        kind: XqTensorType,
        tokens: usize,
        rows: usize,
        columns: usize,
        budget: u64,
    ) -> Result<Self> {
        let invalid = |reason: &str| PowerError::InvalidRequest(reason.into());
        if capability != (9, 0) {
            return Err(PowerError::BackendNotAvailable(
                "XQ SM90 control requires actual compute capability 9.0".into(),
            ));
        }
        if matches!(kind, XqTensorType::I8 | XqTensorType::I32) {
            return Err(invalid("XQ integer metadata cannot enter CUDA projection"));
        }
        let tokens = u32::try_from(tokens).map_err(|_| invalid("XQ token count exceeds u32"))?;
        let rows = u32::try_from(rows).map_err(|_| invalid("XQ output width exceeds u32"))?;
        let columns = u32::try_from(columns).map_err(|_| invalid("XQ input width exceeds u32"))?;
        // The kernel advances a u32 column by 32; leave room for the final increment.
        if tokens == 0 || rows == 0 || columns == 0 || columns > u32::MAX - 31 {
            return Err(invalid(
                "XQ CUDA dimensions are empty or exceed the loop ABI",
            ));
        }
        let row_bytes = u32::try_from(kind.row_bytes(u64::from(columns))?)
            .map_err(|_| invalid("XQ packed row exceeds u32"))?;
        let outputs = u64::from(tokens) * u64::from(rows);
        let grid = outputs.div_ceil(4);
        let output_bytes = outputs
            .checked_mul(4)
            .ok_or_else(|| invalid("XQ CUDA output size overflowed"))?;
        usize::try_from(output_bytes)
            .map_err(|_| invalid("XQ CUDA output exceeds the host address range"))?;
        if grid > i32::MAX as u64 || output_bytes > budget {
            return Err(invalid("XQ CUDA output/grid exceeds its explicit budget"));
        }
        Ok(Self {
            tokens,
            rows,
            columns,
            type_id: kind.gguf_id(),
            row_bytes,
            grid_x: grid as u32,
            output_bytes,
        })
    }
}

#[cfg(any(feature = "cuda", test))]
#[derive(Clone, Copy, Debug)]
pub(crate) struct XqGatherPlan {
    pub projection: XqPlan,
    pub grid_x: u32,
    pub output_bytes: u64,
}

#[cfg(any(feature = "cuda", test))]
impl XqGatherPlan {
    pub(crate) fn new(
        capability: (i32, i32),
        kind: XqTensorType,
        tokens: usize,
        rows: usize,
        columns: usize,
        budget: u64,
    ) -> Result<Self> {
        // Reuse the checked dtype, row-byte and ABI dimension admission. Its
        // dot output bound is not the gather's [tokens,columns] output bound.
        let projection = XqPlan::new(capability, kind, 1, rows, columns, u64::MAX)?;
        let tokens = u32::try_from(tokens)
            .map_err(|_| PowerError::InvalidRequest("XQ gather token count exceeds u32".into()))?;
        let elements = u64::from(tokens) * u64::from(projection.columns);
        let output_bytes = elements
            .checked_mul(4)
            .ok_or_else(|| PowerError::InvalidRequest("XQ gather output size overflowed".into()))?;
        let grid = elements.div_ceil(256);
        if tokens == 0
            || grid > i32::MAX as u64
            || output_bytes > budget
            || usize::try_from(output_bytes).is_err()
        {
            return Err(PowerError::InvalidRequest(
                "XQ gather output/grid exceeds its explicit bound".into(),
            ));
        }
        Ok(Self {
            projection: XqPlan {
                tokens,
                ..projection
            },
            grid_x: grid as u32,
            output_bytes,
        })
    }
}

#[cfg(feature = "cuda")]
pub(crate) mod execution;

#[cfg(test)]
mod tests;
