//! Explicit native projection profiles. The SM90 profile consumes checkpoint
//! bytes directly; it never promotes MXFP4 weights into a resident BF16 copy.
//! Tensor control is a separately requested profile, not an automatic fallback.

#[cfg(any(feature = "cuda", test))]
use aster_power::error::{PowerError, Result};
use sha2::{Digest, Sha256};

pub mod activation;
pub mod finite;
#[cfg(any(feature = "cuda", test))]
pub(crate) mod input;
pub mod sinkhorn;
pub mod xq;

#[cfg(any(feature = "cuda", test))]
use crate::weights::MatrixLayout;

pub const SM90_ABI: &str = "aster-sm90-group32-mma-v1";
pub const SM90_SCALAR_ABI: &str = "aster-sm90-raw-f32-v1";
pub const SM90_COMPILE_OPTIONS: &str = "compute_90;ftz=false;fmad=false;fast_math=false";
pub const SM90_SOURCE: &str = include_str!("cuda/group32.cu");

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProjectionProfile {
    Sm90Group32MmaV1,
    Sm90ScalarF32V1,
    TensorControlV1,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProjectionReceipt {
    pub profile: ProjectionProfile,
    pub abi: &'static str,
    pub source_sha256: String,
    pub ptx_sha256: Option<String>,
    pub compile_options: &'static str,
    pub kernel_attributes: Option<KernelAttributes>,
    /// Additional device workspace, excluding immutable inputs and output.
    pub scratch_bytes: u64,
    /// Raw F32 accumulation before a separately requested BF16 conversion.
    pub kernel_output_bytes: u64,
    /// Bytes in the returned tensor. Neither field is a measured GPU peak.
    pub output_bytes: u64,
}

pub fn source_sha256() -> String {
    format!("{:x}", Sha256::digest(SM90_SOURCE.as_bytes()))
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct KernelAttributes {
    pub registers_per_thread: i32,
    pub local_bytes_per_thread: i32,
    pub static_shared_bytes: i32,
    pub binary_version: i32,
    pub ptx_version: i32,
}

#[cfg(any(feature = "cuda", test))]
#[derive(Clone, Copy, Debug)]
pub(crate) struct MmaPlan {
    pub tokens: u32,
    pub rows: u32,
    pub columns: u32,
    pub packed_fp4: u32,
    pub output_bytes: u64,
}

#[cfg(any(feature = "cuda", test))]
impl MmaPlan {
    pub fn new(
        capability: (i32, i32),
        layout: MatrixLayout,
        tokens: usize,
        rows: usize,
        columns: usize,
        available_bytes: u64,
    ) -> Result<Self> {
        let refuse = |reason: &str| PowerError::InvalidRequest(reason.into());
        if capability != (9, 0) {
            return Err(PowerError::BackendNotAvailable(
                "SM90 projection requires actual compute capability 9.0; no implicit fallback"
                    .into(),
            ));
        }
        let packed_fp4 = match layout {
            MatrixLayout::Fp8Block32 => 0,
            MatrixLayout::Mxfp4Block32 => 1,
            _ => {
                return Err(refuse(
                    "SM90 projection accepts only FP8 block-32 or MXFP4 block-32 weights",
                ))
            }
        };
        if tokens == 0
            || rows == 0
            || columns == 0
            || !columns.is_multiple_of(32)
            || tokens.div_ceil(16) > 65535
            || rows.div_ceil(8) > i32::MAX as usize
        {
            return Err(refuse(
                "SM90 projection geometry exceeds the group-32 kernel ABI",
            ));
        }
        let tokens = u32::try_from(tokens).map_err(|_| refuse("SM90 token count exceeds u32"))?;
        let rows = u32::try_from(rows).map_err(|_| refuse("SM90 output width exceeds u32"))?;
        let columns = u32::try_from(columns).map_err(|_| refuse("SM90 input width exceeds u32"))?;
        let output_bytes = u64::from(tokens) * u64::from(rows) * 4;
        if output_bytes > available_bytes {
            return Err(refuse(
                "SM90 projection output exceeds its live-tensor budget",
            ));
        }
        Ok(Self {
            tokens,
            rows,
            columns,
            packed_fp4,
            output_bytes,
        })
    }

    /// Keep decoded operands and group products in the normal F32 range.
    /// Extreme checkpoint exponents require an explicitly selected scalar or
    /// tensor control profile, including Engram's meaningful exponent code zero.
    pub fn validate_scales(a: (u8, u8), w: (u8, u8)) -> Result<()> {
        if a.0 > a.1
            || w.0 > w.1
            || a.0 < 16
            || w.0 < 16
            || a.1 > 239
            || w.1 > 239
            || u16::from(a.0) + u16::from(w.0) < 146
            || u16::from(a.1) + u16::from(w.1) > 358
        {
            return Err(PowerError::InvalidRequest(
                "SM90 MMA scale range is outside the v1 supported envelope; explicitly select Sm90ScalarF32V1 or TensorControlV1".into(),
            ));
        }
        Ok(())
    }

    pub fn receipt(self) -> ProjectionReceipt {
        ProjectionReceipt {
            profile: ProjectionProfile::Sm90Group32MmaV1,
            abi: SM90_ABI,
            source_sha256: source_sha256(),
            ptx_sha256: None,
            compile_options: SM90_COMPILE_OPTIONS,
            kernel_attributes: None,
            scratch_bytes: 0,
            kernel_output_bytes: self.output_bytes,
            output_bytes: self.output_bytes,
        }
    }
}

#[cfg(feature = "cuda")]
pub(crate) mod execution;

#[cfg(test)]
mod tests;
