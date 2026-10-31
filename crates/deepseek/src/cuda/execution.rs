//! Candle/cudarc bridge for ABI v1. All allocations retain normal stream event
//! tracking. No raw pointer escapes or external allocator/stream is introduced.

use std::sync::OnceLock;

use aster_power::error::{PowerError, Result};
use candle_core::cuda_backend::{cudarc, WrapErr};
use candle_core::{CpuStorage, CudaStorage, CustomOp1, Layout, Shape, Storage, Tensor};
use cudarc::driver::{LaunchConfig, PushKernelArg};
use sha2::{Digest, Sha256};

use super::{
    KernelAttributes, MmaPlan, ProjectionProfile, ProjectionReceipt, SM90_ABI, SM90_SCALAR_ABI,
    SM90_SOURCE,
};
use crate::layers::{tensor_error, CudaLayerDevice};
use crate::weights::MatrixLayout;

static PTX: OnceLock<std::result::Result<String, String>> = OnceLock::new();

fn ptx() -> candle_core::Result<&'static str> {
    match PTX.get_or_init(|| {
        cudarc::nvrtc::compile_ptx_with_opts(
            SM90_SOURCE,
            cudarc::nvrtc::CompileOptions {
                arch: Some("compute_90"),
                ftz: Some(false),
                fmad: Some(false),
                use_fast_math: Some(false),
                ..Default::default()
            },
        )
        .map(|p| p.to_src())
        .map_err(|e| format!("SM90 NVRTC compilation failed: {e}"))
    }) {
        Ok(ptx) => Ok(ptx),
        Err(error) => candle_core::bail!("{error}"),
    }
}

pub(crate) fn project(
    device: &CudaLayerDevice,
    profile: ProjectionProfile,
    layout: MatrixLayout,
    a: &Tensor,
    asf: &Tensor,
    w: &Tensor,
    wsf: &Tensor,
    rows: usize,
) -> Result<(Tensor, ProjectionReceipt)> {
    let capability = device
        .tensor_device()
        .as_cuda_device()
        .map_err(tensor_error)?
        .cuda_stream()
        .context()
        .compute_capability()
        .map_err(|e| PowerError::InferenceFailed(format!("CUDA capability query failed: {e}")))?;
    let (tokens, columns) = a.dims2().map_err(tensor_error)?;
    let plan = MmaPlan::new(
        capability,
        layout,
        tokens,
        rows,
        columns,
        device.limits().max_state_bytes,
    )?;
    let range = |t: &Tensor| -> Result<(u8, u8)> {
        Ok((
            t.min_all()
                .and_then(|v| v.to_scalar::<u8>())
                .map_err(tensor_error)?,
            t.max_all()
                .and_then(|v| v.to_scalar::<u8>())
                .map_err(tensor_error)?,
        ))
    };
    match profile {
        ProjectionProfile::Sm90Group32MmaV1 => MmaPlan::validate_scales(range(asf)?, range(wsf)?)?,
        ProjectionProfile::Sm90ScalarF32V1 => {}
        ProjectionProfile::TensorControlV1 => {
            return Err(PowerError::InvalidRequest(
                "tensor control is not a custom SM90 kernel".into(),
            ))
        }
    }
    let cuda = device
        .tensor_device()
        .as_cuda_device()
        .map_err(tensor_error)?;
    let func = cuda
        .get_or_load_custom_func(
            function_name(profile),
            SM90_ABI,
            ptx().map_err(tensor_error)?,
        )
        .map_err(tensor_error)?;
    let query = |r: std::result::Result<i32, cudarc::driver::DriverError>| {
        r.map_err(|e| {
            PowerError::InferenceFailed(format!("SM90 kernel attribute query failed: {e}"))
        })
    };
    let attributes = KernelAttributes {
        registers_per_thread: query(func.num_regs())?,
        local_bytes_per_thread: query(func.local_size_bytes())?,
        static_shared_bytes: query(func.shared_size_bytes())?,
        binary_version: query(func.binary_version())?,
        ptx_version: query(func.ptx_version())?,
    };
    if attributes.local_bytes_per_thread != 0 || attributes.static_shared_bytes != 0 {
        return Err(PowerError::BackendNotAvailable(
            "SM90 v1 zero-workspace kernel has unexpected local/shared allocation".into(),
        ));
    }
    // A column-range view can have strides. Bound all potentially copied raw
    // tensors plus the F32 result before making any contiguous snapshots.
    let bytes = [a, asf, w, wsf]
        .iter()
        .try_fold(plan.output_bytes, |sum, t| {
            sum.checked_add(t.elem_count() as u64)
                .ok_or_else(|| PowerError::InvalidRequest("SM90 live byte bound overflowed".into()))
        })?;
    device
        .limits()
        .checked_state_bytes(bytes, "SM90 contiguous inputs and output")?;
    let a = a.force_contiguous().map_err(tensor_error)?;
    let op = MmaOp {
        plan,
        profile,
        asf: asf.force_contiguous().map_err(tensor_error)?,
        w: w.force_contiguous().map_err(tensor_error)?,
        wsf: wsf.force_contiguous().map_err(tensor_error)?,
    };
    let result = a.apply_op1_no_bwd(&op).map_err(tensor_error)?;
    let mut receipt = plan.receipt();
    receipt.profile = profile;
    receipt.kernel_attributes = Some(attributes);
    if profile == ProjectionProfile::Sm90ScalarF32V1 {
        receipt.abi = SM90_SCALAR_ABI;
    }
    receipt.ptx_sha256 = Some(format!(
        "{:x}",
        Sha256::digest(ptx().map_err(tensor_error)?.as_bytes())
    ));
    Ok((result, receipt))
}

struct MmaOp {
    plan: MmaPlan,
    profile: ProjectionProfile,
    asf: Tensor,
    w: Tensor,
    wsf: Tensor,
}

fn function_name(profile: ProjectionProfile) -> &'static str {
    if profile == ProjectionProfile::Sm90ScalarF32V1 {
        "aster_raw_f32_v1"
    } else {
        "aster_group32_mma_v1"
    }
}

impl CustomOp1 for MmaOp {
    fn name(&self) -> &'static str {
        if self.profile == ProjectionProfile::Sm90ScalarF32V1 {
            SM90_SCALAR_ABI
        } else {
            SM90_ABI
        }
    }

    fn cpu_fwd(&self, _: &CpuStorage, _: &Layout) -> candle_core::Result<(CpuStorage, Shape)> {
        candle_core::bail!("SM90 MMA has no CPU implementation or fallback")
    }

    fn cuda_fwd(&self, a: &CudaStorage, al: &Layout) -> candle_core::Result<(CudaStorage, Shape)> {
        let device = &a.device;
        let name = function_name(self.profile);
        let func = device.get_or_load_custom_func(name, SM90_ABI, ptx()?)?;
        let view = |l: &Layout| {
            l.contiguous_offsets().ok_or_else(|| {
                candle_core::Error::Msg("SM90 ABI requires contiguous storage".into())
            })
        };
        let (start, end) = view(al)?;
        let a = a.as_cuda_slice::<u8>()?.slice(start..end);
        let (asg, asl) = self.asf.storage_and_layout();
        let (wg, wl) = self.w.storage_and_layout();
        let (wsg, wsl) = self.wsf.storage_and_layout();
        let (Storage::Cuda(ass), Storage::Cuda(ws), Storage::Cuda(wss)) = (&*asg, &*wg, &*wsg)
        else {
            candle_core::bail!("SM90 ABI requires CUDA storage for every operand")
        };
        let (start, end) = view(asl)?;
        let asf = ass.as_cuda_slice::<u8>()?.slice(start..end);
        let (start, end) = view(wl)?;
        let w = ws.as_cuda_slice::<u8>()?.slice(start..end);
        let (start, end) = view(wsl)?;
        let wsf = wss.as_cuda_slice::<u8>()?.slice(start..end);
        let stream = device.cuda_stream();
        let p = self.plan;
        // SAFETY: plan validates dimensions/grid and output byte bound; every
        // output element is written once, including partial 16x8 edge tiles.
        // Input views cover immutable validated U8 tensors. Cudarc retains
        // stream events for all borrowed slices until their use completes.
        let mut output = unsafe { stream.alloc::<f32>(p.tokens as usize * p.rows as usize) }.w()?;
        let mut builder = func.builder();
        builder
            .arg(&a)
            .arg(&asf)
            .arg(&w)
            .arg(&wsf)
            .arg(&mut output)
            .arg(&p.tokens)
            .arg(&p.rows)
            .arg(&p.columns)
            .arg(&p.packed_fp4);
        unsafe {
            builder.launch(LaunchConfig {
                grid_dim: (p.rows.div_ceil(8), p.tokens.div_ceil(16), 1),
                block_dim: (32, 1, 1),
                shared_mem_bytes: 0,
            })
        }
        .w()?;
        Ok((
            CudaStorage::wrap_cuda_slice(output, device.clone()),
            Shape::from((p.tokens as usize, p.rows as usize)),
        ))
    }
}
