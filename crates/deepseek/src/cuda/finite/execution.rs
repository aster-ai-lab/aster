use super::{geometry, ABI, SOURCE};
use crate::layers::tensor_error;
use aster_power::error::{PowerError, Result};
use candle_core::cuda_backend::{cudarc, WrapErr};
use candle_core::{CpuStorage, CudaStorage, CustomOp1, DType, Layout, Shape, Tensor};
use cudarc::driver::{LaunchConfig, PushKernelArg};
use std::sync::OnceLock;

static PTX: OnceLock<std::result::Result<String, String>> = OnceLock::new();
fn ptx() -> candle_core::Result<&'static str> {
    match PTX.get_or_init(|| {
        cudarc::nvrtc::compile_ptx_with_opts(
            SOURCE,
            cudarc::nvrtc::CompileOptions {
                arch: Some("compute_90"),
                ftz: Some(false),
                fmad: Some(false),
                use_fast_math: Some(false),
                ..Default::default()
            },
        )
        .map(|x| x.to_src())
        .map_err(|e| format!("finite flag NVRTC failed: {e}"))
    }) {
        Ok(value) => Ok(value),
        Err(error) => candle_core::bail!("{error}"),
    }
}

pub(crate) fn validate(tensor: &Tensor, nonnegative: bool, label: &str) -> Result<()> {
    validate_with_input_policy(tensor, nonnegative, label, crate::cuda::input::POLICY)
}

pub(super) fn validate_with_input_policy(
    tensor: &Tensor,
    nonnegative: bool,
    label: &str,
    policy: crate::cuda::input::Policy,
) -> Result<()> {
    if !matches!(tensor.dtype(), DType::F32 | DType::BF16) {
        return Err(PowerError::InvalidRequest(
            "finite validation requires FP32 or BF16 data".into(),
        ));
    }
    let (elements, grid) = geometry(tensor.elem_count())?;
    let device = tensor.device().as_cuda_device().map_err(tensor_error)?;
    let capability = device
        .cuda_stream()
        .context()
        .compute_capability()
        .map_err(|e| {
            PowerError::InferenceFailed(format!("finite flag capability query failed: {e}"))
        })?;
    if capability != (9, 0) {
        return Err(PowerError::BackendNotAvailable(
            "fused finite validation requires actual SM90".into(),
        ));
    }
    let input = tensor
        .to_dtype(DType::F32)
        .and_then(|x| crate::cuda::input::prepare_with_policy(&x, policy))
        .map_err(tensor_error)?;
    let flag = input
        .apply_op1_no_bwd(&Op {
            elements,
            grid,
            nonnegative: u32::from(nonnegative),
        })
        .and_then(|x| x.to_scalar::<u32>())
        .map_err(tensor_error)?;
    if flag != 0 {
        return Err(PowerError::InvalidRequest(format!(
            "{label} contains nonfinite or forbidden negative values"
        )));
    }
    Ok(())
}

struct Op {
    elements: u32,
    grid: u32,
    nonnegative: u32,
}
impl CustomOp1 for Op {
    fn name(&self) -> &'static str {
        ABI
    }
    fn cpu_fwd(&self, _: &CpuStorage, _: &Layout) -> candle_core::Result<(CpuStorage, Shape)> {
        candle_core::bail!("fused finite flag has no CPU fallback")
    }
    fn cuda_fwd(
        &self,
        input: &CudaStorage,
        layout: &Layout,
    ) -> candle_core::Result<(CudaStorage, Shape)> {
        let device = &input.device;
        let function = device.get_or_load_custom_func("aster_finite_f32_flag_v1", ABI, ptx()?)?;
        let (first, last) = layout
            .contiguous_offsets()
            .ok_or_else(|| candle_core::Error::Msg("finite input must be contiguous".into()))?;
        let input = input.as_cuda_slice::<f32>()?.slice(first..last);
        let stream = device.cuda_stream();
        let mut flag = stream.alloc_zeros::<u32>(1).w()?;
        let mut builder = function.builder();
        builder
            .arg(&input)
            .arg(&mut flag)
            .arg(&self.elements)
            .arg(&self.nonnegative);
        // SAFETY: geometry checks the complete u32 count; the kernel uses a
        // u64 index and never reads a tail/empty element. All warps remain live
        // for ballot. A single zero-initialized U32 flag has only atomic writers.
        // Cudarc tracks all input/output stream events and storage lifetimes.
        unsafe {
            builder.launch(LaunchConfig {
                grid_dim: (self.grid, 1, 1),
                block_dim: (256, 1, 1),
                shared_mem_bytes: 0,
            })
        }
        .w()?;
        Ok((
            CudaStorage::wrap_cuda_slice(flag, device.clone()),
            Shape::from(()),
        ))
    }
}
