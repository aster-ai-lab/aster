use super::{Plan, ABI, SOURCE};
use crate::layers::{tensor_error, validate_finite};
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
        .map(|value| value.to_src())
        .map_err(|error| format!("fused XQ Sinkhorn NVRTC failed: {error}"))
    }) {
        Ok(value) => Ok(value),
        Err(error) => candle_core::bail!("{error}"),
    }
}

pub(crate) fn normalize(input: &Tensor, budget: u64) -> Result<Tensor> {
    if input.dtype() != DType::F32 {
        return Err(PowerError::InvalidRequest(
            "fused XQ Sinkhorn accepts only unchanged F32 input".into(),
        ));
    }
    let device = input.device().as_cuda_device().map_err(tensor_error)?;
    let capability = device
        .cuda_stream()
        .context()
        .compute_capability()
        .map_err(|error| {
            PowerError::InferenceFailed(format!("Sinkhorn capability query failed: {error}"))
        })?;
    let plan = Plan::new(capability, input.dims(), budget)?;
    validate_finite(input, true, "fused XQ Sinkhorn input")?;
    if input
        .max_all()
        .and_then(|x| x.to_scalar::<f32>())
        .map_err(tensor_error)?
        > 2.
    {
        return Err(PowerError::InvalidRequest(
            "fused XQ Sinkhorn input exceeds the admitted softmax/epsilon range".into(),
        ));
    }
    // In [0,2], every four-element sum is <=8 and every denominator is
    // >=epsilon. Later elements are <=1. No intermediate can overflow or
    // divide by zero; validation is retained on both public boundaries.
    let input = input.force_contiguous().map_err(tensor_error)?;
    let output = input.apply_op1_no_bwd(&Op(plan)).map_err(tensor_error)?;
    validate_finite(&output, true, "fused XQ Sinkhorn output")?;
    Ok(output)
}

struct Op(Plan);

impl CustomOp1 for Op {
    fn name(&self) -> &'static str {
        ABI
    }

    fn cpu_fwd(&self, _: &CpuStorage, _: &Layout) -> candle_core::Result<(CpuStorage, Shape)> {
        candle_core::bail!("fused XQ Sinkhorn has no CPU fallback")
    }

    fn cuda_fwd(
        &self,
        input: &CudaStorage,
        layout: &Layout,
    ) -> candle_core::Result<(CudaStorage, Shape)> {
        let device = &input.device;
        let function = device.get_or_load_custom_func("aster_xq_sinkhorn4_f32_v1", ABI, ptx()?)?;
        let (first, last) = layout
            .contiguous_offsets()
            .ok_or_else(|| candle_core::Error::Msg("Sinkhorn input must be contiguous".into()))?;
        let input = input.as_cuda_slice::<f32>()?.slice(first..last);
        let stream = device.cuda_stream();
        // SAFETY: Plan bounds every input/output and grid index. Each admitted
        // warp owns one 16-element matrix, with mask 0xffff for its live lanes.
        // Cudarc tracks borrowed storage and output events across streams.
        let mut output = unsafe { stream.alloc::<f32>((self.0.output_bytes / 4) as usize) }.w()?;
        let mut builder = function.builder();
        builder.arg(&input).arg(&mut output).arg(&self.0.tokens);
        unsafe {
            builder.launch(LaunchConfig {
                grid_dim: (self.0.grid, 1, 1),
                block_dim: (128, 1, 1),
                shared_mem_bytes: 0,
            })
        }
        .w()?;
        Ok((
            CudaStorage::wrap_cuda_slice(output, device.clone()),
            Shape::from((self.0.tokens as usize, 4, 4)),
        ))
    }
}
