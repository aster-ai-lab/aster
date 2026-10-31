//! Model-owned native tensor layers. Public construction requires an actual
//! CUDA device; CPU math used by sibling tests is never a serving fallback.

use aster_power::error::{PowerError, Result};
use aster_power::inference::{InferenceLimits, RuntimeDevice, RuntimeDeviceKind};

pub mod attention;
pub mod compressor;
pub mod decoder;
pub mod engram;
pub mod indexer;
pub mod kv;
pub mod linear;
pub mod mhc;
pub mod moe;
pub mod norm;
pub mod quant;
pub mod rope;
pub mod sublayer;
pub mod xq;

#[derive(Clone)]
pub struct CudaLayerDevice {
    runtime: RuntimeDevice,
    limits: InferenceLimits,
    projection_profile: crate::cuda::ProjectionProfile,
}

impl CudaLayerDevice {
    #[cfg(test)]
    pub(crate) fn cpu_for_tests(limits: InferenceLimits) -> Self {
        // The helper is absent from production builds. Public admission still
        // requires actual CUDA; CPU fixtures exercise only arithmetic/state.
        Self {
            runtime: RuntimeDevice::resolve(aster_power::inference::DevicePreference::Cpu).unwrap(),
            limits,
            projection_profile: crate::cuda::ProjectionProfile::TensorControlV1,
        }
    }

    pub fn new(runtime: RuntimeDevice, limits: InferenceLimits) -> Result<Self> {
        limits.validate()?;
        if !cfg!(feature = "cuda") || runtime.kind() != RuntimeDeviceKind::Cuda {
            return Err(PowerError::BackendNotAvailable(
                "native tensor layers require the cuda feature and an actual CUDA device".into(),
            ));
        }
        Ok(Self {
            runtime,
            limits,
            projection_profile: crate::cuda::ProjectionProfile::Sm90Group32MmaV1,
        })
    }

    /// An explicit model execution choice; never selected after an MMA error.
    pub fn with_projection_profile(mut self, profile: crate::cuda::ProjectionProfile) -> Self {
        self.projection_profile = profile;
        self
    }

    pub fn projection_profile(&self) -> crate::cuda::ProjectionProfile {
        self.projection_profile
    }

    pub(crate) fn tensor_device(&self) -> &candle_core::Device {
        self.runtime.tensor_device()
    }

    pub(crate) fn limits(&self) -> &InferenceLimits {
        &self.limits
    }
}

pub(crate) fn tensor_error(error: candle_core::Error) -> PowerError {
    PowerError::InferenceFailed(format!("native DeepSeek tensor operation failed: {error}"))
}

/// Synchronize one bounded validation status, without reading tensor payloads
/// back to the host. Kernel fusion can later provide the same status directly.
pub(crate) fn validate_finite(
    tensor: &candle_core::Tensor,
    nonnegative: bool,
    label: &str,
) -> Result<()> {
    #[cfg(feature = "xq-validation-cache")]
    return crate::execution::validation_cache::validate(tensor, nonnegative, || {
        validate_finite_uncached(tensor, nonnegative, label)
    });
    #[cfg(not(feature = "xq-validation-cache"))]
    validate_finite_uncached(tensor, nonnegative, label)
}

fn validate_finite_uncached(
    tensor: &candle_core::Tensor,
    nonnegative: bool,
    label: &str,
) -> Result<()> {
    #[cfg(feature = "xq-fused-validation")]
    if tensor.device().is_cuda() {
        return crate::cuda::finite::validate(tensor, nonnegative, label);
    }
    validate_finite_tensor(tensor, nonnegative, label)
}

pub(crate) fn validate_finite_tensor(
    tensor: &candle_core::Tensor,
    nonnegative: bool,
    label: &str,
) -> Result<()> {
    use candle_core::DType;
    if !matches!(tensor.dtype(), DType::F32 | DType::BF16) {
        return Err(invalid("finite validation requires FP32 or BF16 data"));
    }
    let expected = u32::try_from(tensor.elem_count())
        .map_err(|_| invalid("validation element count exceeds u32"))?;
    let check = || -> candle_core::Result<u32> {
        let tensor = tensor.to_dtype(DType::F32)?;
        let valid = tensor.abs()?.le(f32::MAX)?.mul(&tensor.eq(&tensor)?)?;
        let valid = if nonnegative {
            valid.mul(&tensor.ge(0_f32)?)?
        } else {
            valid
        };
        valid.to_dtype(DType::U32)?.sum_all()?.to_scalar::<u32>()
    };
    if check().map_err(tensor_error)? != expected {
        return Err(PowerError::InvalidRequest(format!(
            "{label} contains nonfinite or forbidden negative values"
        )));
    }
    Ok(())
}

fn invalid(message: &str) -> PowerError {
    PowerError::InvalidRequest(message.into())
}
