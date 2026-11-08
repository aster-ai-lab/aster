use super::{Kind, Plan, ABI, SOURCE};
use candle_core::cuda_backend::{cudarc, WrapErr};
use candle_core::{CpuStorage, CudaStorage, CustomOp1, Layout, Shape, Tensor};
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
        .map_err(|e| format!("activation codec NVRTC failed: {e}"))
    }) {
        Ok(source) => Ok(source),
        Err(error) => candle_core::bail!("{error}"),
    }
}

pub(crate) fn encode_e4m3(input: &Tensor) -> candle_core::Result<Tensor> {
    run(input, Kind::EncodeE4m3)
}
pub(crate) fn decode_e4m3(input: &Tensor) -> candle_core::Result<Tensor> {
    run(input, Kind::DecodeE4m3)
}
pub(crate) fn encode_e2m1(input: &Tensor) -> candle_core::Result<Tensor> {
    run(input, Kind::EncodeE2m1)
}
pub(crate) fn decode_e2m1(input: &Tensor) -> candle_core::Result<Tensor> {
    run(input, Kind::DecodeE2m1)
}

fn run(input: &Tensor, kind: Kind) -> candle_core::Result<Tensor> {
    let plan = Plan::new(kind, input.shape(), input.dtype())?;
    let device = input.device().as_cuda_device()?;
    let capability = device.cuda_stream().context().compute_capability().w()?;
    if capability != (9, 0) {
        candle_core::bail!("fused activation codec requires actual SM90; no fallback");
    }
    crate::cuda::input::prepare(input)?.apply_op1_no_bwd(&Op { kind, plan })
}

struct Op {
    kind: Kind,
    plan: Plan,
}
impl CustomOp1 for Op {
    fn name(&self) -> &'static str {
        ABI
    }
    fn cpu_fwd(&self, _: &CpuStorage, _: &Layout) -> candle_core::Result<(CpuStorage, Shape)> {
        candle_core::bail!("fused activation codec has no CPU fallback")
    }
    fn cuda_fwd(
        &self,
        input: &CudaStorage,
        layout: &Layout,
    ) -> candle_core::Result<(CudaStorage, Shape)> {
        let device = &input.device;
        let symbol = match self.kind {
            Kind::EncodeE4m3 => "aster_encode_e4m3_f32_v1",
            Kind::DecodeE4m3 => "aster_decode_e4m3_f32_v1",
            Kind::EncodeE2m1 => "aster_encode_e2m1_f32_v1",
            Kind::DecodeE2m1 => "aster_decode_e2m1_f32_v1",
        };
        let function = device.get_or_load_custom_func(symbol, ABI, ptx()?)?;
        let (first, last) = layout
            .contiguous_offsets()
            .ok_or_else(|| candle_core::Error::Msg("activation input must be contiguous".into()))?;
        let stream = device.cuda_stream();
        let config = LaunchConfig {
            grid_dim: (self.plan.grid, 1, 1),
            block_dim: (256, 1, 1),
            shared_mem_bytes: 0,
        };
        let output = match self.kind {
            Kind::EncodeE4m3 | Kind::EncodeE2m1 => {
                let input = input.as_cuda_slice::<f32>()?.slice(first..last);
                let mut output = unsafe { stream.alloc::<u8>(self.plan.output_elements) }.w()?;
                let mut builder = function.builder();
                builder
                    .arg(&input)
                    .arg(&mut output)
                    .arg(&self.plan.work_items);
                // SAFETY: the checked plan covers each output exactly once;
                // FP4 reads two adjacent input elements per output byte. U64
                // indices prevent overflow at grid tails. Cudarc tracks the
                // input/output events and storage lifetime on the bound stream.
                unsafe { builder.launch(config) }.w()?;
                CudaStorage::wrap_cuda_slice(output, device.clone())
            }
            Kind::DecodeE4m3 | Kind::DecodeE2m1 => {
                let input = input.as_cuda_slice::<u8>()?.slice(first..last);
                let mut output = unsafe { stream.alloc::<f32>(self.plan.output_elements) }.w()?;
                let mut builder = function.builder();
                builder
                    .arg(&input)
                    .arg(&mut output)
                    .arg(&self.plan.work_items);
                // SAFETY: each checked input byte owns one or two disjoint
                // output F32 elements, including zero-length/tail geometry.
                // The kernel never reads beyond the contiguous view.
                unsafe { builder.launch(config) }.w()?;
                CudaStorage::wrap_cuda_slice(output, device.clone())
            }
        };
        Ok((output, self.plan.output_shape.clone()))
    }
}
