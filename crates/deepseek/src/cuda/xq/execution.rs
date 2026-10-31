//! Event-tracked Candle/cudarc bridge for packed XQ math control.

use std::sync::OnceLock;

use aster_power::error::{PowerError, Result};
use candle_core::cuda_backend::{cudarc, WrapErr};
use candle_core::{CpuStorage, CudaStorage, CustomOp1, DType, Layout, Shape, Storage, Tensor};
use cudarc::driver::{LaunchConfig, PushKernelArg};
use sha2::{Digest, Sha256};

use super::{
    source_sha256, XqGatherPlan, XqPlan, XqProjectionReceipt, XQ_CUDA_ABI, XQ_CUDA_OPTIONS,
    XQ_CUDA_SOURCE, XQ_GATHER_ABI,
};
use crate::cuda::KernelAttributes;
use crate::layers::{tensor_error, validate_finite, CudaLayerDevice};
use crate::weights::xq::XqTensorType;

static PTX: OnceLock<std::result::Result<String, String>> = OnceLock::new();

fn ptx() -> candle_core::Result<&'static str> {
    match PTX.get_or_init(|| {
        cudarc::nvrtc::compile_ptx_with_opts(
            XQ_CUDA_SOURCE,
            cudarc::nvrtc::CompileOptions {
                arch: Some("compute_90"),
                ftz: Some(false),
                fmad: Some(false),
                use_fast_math: Some(false),
                ..Default::default()
            },
        )
        .map(|ptx| ptx.to_src())
        .map_err(|e| format!("XQ SM90 NVRTC compilation failed: {e}"))
    }) {
        Ok(ptx) => Ok(ptx),
        Err(error) => candle_core::bail!("{error}"),
    }
}

pub(crate) fn project(
    device: &CudaLayerDevice,
    kind: XqTensorType,
    shape: [usize; 2],
    packed: &Tensor,
    input: &Tensor,
) -> Result<(Tensor, XqProjectionReceipt)> {
    let cuda = device
        .tensor_device()
        .as_cuda_device()
        .map_err(tensor_error)?;
    let capability = cuda
        .cuda_stream()
        .context()
        .compute_capability()
        .map_err(|e| {
            PowerError::InferenceFailed(format!("XQ CUDA capability query failed: {e}"))
        })?;
    let (tokens, columns) = input.dims2().map_err(tensor_error)?;
    if columns != shape[1]
        || input.dtype() != DType::F32
        || packed.dtype() != DType::U8
        || packed.dims() != [shape[0], kind.row_bytes(columns as u64)? as usize]
        || !input.device().same_device(device.tensor_device())
        || !packed.device().same_device(input.device())
    {
        return Err(PowerError::InvalidRequest(
            "XQ F32 control operand shape/dtype/device differs from its bound matrix".into(),
        ));
    }
    let plan = XqPlan::new(
        capability,
        kind,
        tokens,
        shape[0],
        columns,
        device.limits().max_state_bytes,
    )?;
    let live = (input.elem_count() as u64)
        .checked_mul(8)
        .and_then(|bytes| {
            plan.output_bytes
                .checked_mul(16)
                .and_then(|output| bytes.checked_add(output))
        })
        .and_then(|bytes| bytes.checked_add(packed.elem_count() as u64))
        .ok_or_else(|| {
            PowerError::InvalidRequest("XQ projection live byte count overflowed".into())
        })?;
    device
        .limits()
        .checked_state_bytes(live, "XQ control operands, output and finite validation")?;
    validate_finite(input, false, "XQ F32 control input")?;
    let func = cuda
        .get_or_load_custom_func(
            "aster_xq_warp_f32_v1",
            XQ_CUDA_ABI,
            ptx().map_err(tensor_error)?,
        )
        .map_err(tensor_error)?;
    let query = |r: std::result::Result<i32, cudarc::driver::DriverError>| {
        r.map_err(|e| PowerError::InferenceFailed(format!("XQ kernel attribute query failed: {e}")))
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
            "XQ zero-workspace control has unexpected local/shared allocation".into(),
        ));
    }
    let input = input.force_contiguous().map_err(tensor_error)?;
    let op = XqOp {
        plan,
        packed: packed.clone(),
    };
    let output = input.apply_op1_no_bwd(&op).map_err(tensor_error)?;
    validate_finite(&output, false, "XQ F32 control output")?;
    let receipt = XqProjectionReceipt {
        abi: XQ_CUDA_ABI,
        source_sha256: source_sha256(),
        ptx_sha256: format!(
            "{:x}",
            Sha256::digest(ptx().map_err(tensor_error)?.as_bytes())
        ),
        compile_options: XQ_CUDA_OPTIONS,
        kernel_attributes: attributes,
        output_bytes: plan.output_bytes,
        scratch_bytes: 0,
    };
    Ok((output, receipt))
}

pub(crate) fn gather(
    device: &CudaLayerDevice,
    kind: XqTensorType,
    shape: [usize; 2],
    packed: &Tensor,
    indices: &Tensor,
) -> Result<(Tensor, XqProjectionReceipt)> {
    let cuda = device
        .tensor_device()
        .as_cuda_device()
        .map_err(tensor_error)?;
    let capability = cuda
        .cuda_stream()
        .context()
        .compute_capability()
        .map_err(|e| {
            PowerError::InferenceFailed(format!("XQ CUDA capability query failed: {e}"))
        })?;
    let tokens = indices.dims1().map_err(tensor_error)?;
    if indices.dtype() != DType::U32
        || !indices.device().same_device(device.tensor_device())
        || packed.dtype() != DType::U8
        || packed.dims() != [shape[0], kind.row_bytes(shape[1] as u64)? as usize]
        || !packed.device().same_device(indices.device())
    {
        return Err(PowerError::InvalidRequest(
            "XQ gather requires same-device U32 indices and exact packed matrix rows".into(),
        ));
    }
    let plan = XqGatherPlan::new(
        capability,
        kind,
        tokens,
        shape[0],
        shape[1],
        device.limits().max_state_bytes,
    )?;
    device
        .limits()
        .checked_elements(&[tokens, shape[1]], "XQ gather decoded rows")?;
    let live = plan
        .output_bytes
        .checked_mul(16)
        .and_then(|bytes| {
            (tokens as u64)
                .checked_mul(8)
                .and_then(|ids| bytes.checked_add(ids))
        })
        .ok_or_else(|| PowerError::InvalidRequest("XQ gather live byte bound overflowed".into()))?;
    device
        .limits()
        .checked_state_bytes(live, "XQ gathered F32 output and finite validation")?;
    let indices = indices.force_contiguous().map_err(tensor_error)?.detach();
    if indices
        .max_all()
        .and_then(|x| x.to_scalar::<u32>())
        .map_err(tensor_error)?
        >= plan.projection.rows
    {
        return Err(PowerError::InvalidRequest(
            "XQ gather index exceeds its loaded matrix rows".into(),
        ));
    }
    let function = cuda
        .get_or_load_custom_func(
            "aster_xq_gather_f32_v1",
            XQ_CUDA_ABI,
            ptx().map_err(tensor_error)?,
        )
        .map_err(tensor_error)?;
    let query = |r: std::result::Result<i32, cudarc::driver::DriverError>| {
        r.map_err(|e| PowerError::InferenceFailed(format!("XQ gather attribute query failed: {e}")))
    };
    let attributes = KernelAttributes {
        registers_per_thread: query(function.num_regs())?,
        local_bytes_per_thread: query(function.local_size_bytes())?,
        static_shared_bytes: query(function.shared_size_bytes())?,
        binary_version: query(function.binary_version())?,
        ptx_version: query(function.ptx_version())?,
    };
    if attributes.local_bytes_per_thread != 0 || attributes.static_shared_bytes != 0 {
        return Err(PowerError::BackendNotAvailable(
            "XQ zero-workspace gather has unexpected local/shared allocation".into(),
        ));
    }
    let output = indices
        .apply_op1_no_bwd(&GatherOp {
            plan,
            packed: packed.clone(),
        })
        .map_err(tensor_error)?;
    validate_finite(&output, false, "XQ gathered F32 rows")?;
    Ok((
        output,
        XqProjectionReceipt {
            abi: XQ_GATHER_ABI,
            source_sha256: source_sha256(),
            ptx_sha256: format!(
                "{:x}",
                Sha256::digest(ptx().map_err(tensor_error)?.as_bytes())
            ),
            compile_options: XQ_CUDA_OPTIONS,
            kernel_attributes: attributes,
            output_bytes: plan.output_bytes,
            scratch_bytes: 0,
        },
    ))
}

struct GatherOp {
    plan: XqGatherPlan,
    packed: Tensor,
}

impl CustomOp1 for GatherOp {
    fn name(&self) -> &'static str {
        XQ_GATHER_ABI
    }
    fn cpu_fwd(&self, _: &CpuStorage, _: &Layout) -> candle_core::Result<(CpuStorage, Shape)> {
        candle_core::bail!("XQ SM90 gather has no CPU fallback")
    }
    fn cuda_fwd(
        &self,
        input: &CudaStorage,
        layout: &Layout,
    ) -> candle_core::Result<(CudaStorage, Shape)> {
        let device = &input.device;
        let function =
            device.get_or_load_custom_func("aster_xq_gather_f32_v1", XQ_CUDA_ABI, ptx()?)?;
        let (start, end) = layout.contiguous_offsets().ok_or_else(|| {
            candle_core::Error::Msg("XQ gather requires contiguous indices".into())
        })?;
        let indices = input.as_cuda_slice::<u32>()?.slice(start..end);
        let (weights, layout) = self.packed.storage_and_layout();
        let Storage::Cuda(weights) = &*weights else {
            candle_core::bail!("XQ gather requires CUDA packed rows")
        };
        let (start, end) = layout.contiguous_offsets().ok_or_else(|| {
            candle_core::Error::Msg("XQ gather requires contiguous packed weights".into())
        })?;
        let weights = weights.as_cuda_slice::<u8>()?.slice(start..end);
        let p = self.plan.projection;
        let stream = device.cuda_stream();
        // SAFETY: admission validates all dimensions, row bytes, output size
        // and immutable U32 row IDs. Each thread writes one bounded element;
        // the kernel also guards malformed IDs before reading weights.
        let mut output = unsafe { stream.alloc::<f32>(self.plan.output_bytes as usize / 4) }.w()?;
        let mut builder = function.builder();
        builder
            .arg(&indices)
            .arg(&weights)
            .arg(&mut output)
            .arg(&p.tokens)
            .arg(&p.rows)
            .arg(&p.columns)
            .arg(&p.type_id)
            .arg(&p.row_bytes);
        unsafe {
            builder.launch(LaunchConfig {
                grid_dim: (self.plan.grid_x, 1, 1),
                block_dim: (256, 1, 1),
                shared_mem_bytes: 0,
            })
        }
        .w()?;
        Ok((
            CudaStorage::wrap_cuda_slice(output, device.clone()),
            Shape::from((p.tokens as usize, p.columns as usize)),
        ))
    }
}

struct XqOp {
    plan: XqPlan,
    packed: Tensor,
}

impl CustomOp1 for XqOp {
    fn name(&self) -> &'static str {
        XQ_CUDA_ABI
    }

    fn cpu_fwd(&self, _: &CpuStorage, _: &Layout) -> candle_core::Result<(CpuStorage, Shape)> {
        candle_core::bail!("XQ SM90 projection has no CPU fallback")
    }

    fn cuda_fwd(
        &self,
        input: &CudaStorage,
        layout: &Layout,
    ) -> candle_core::Result<(CudaStorage, Shape)> {
        let device = &input.device;
        let func = device.get_or_load_custom_func("aster_xq_warp_f32_v1", XQ_CUDA_ABI, ptx()?)?;
        let (start, end) = layout
            .contiguous_offsets()
            .ok_or_else(|| candle_core::Error::Msg("XQ CUDA requires contiguous input".into()))?;
        let input = input.as_cuda_slice::<f32>()?.slice(start..end);
        let (weights, wl) = self.packed.storage_and_layout();
        let Storage::Cuda(weights) = &*weights else {
            candle_core::bail!("XQ CUDA requires packed CUDA storage")
        };
        let (start, end) = wl
            .contiguous_offsets()
            .ok_or_else(|| candle_core::Error::Msg("XQ CUDA requires contiguous weights".into()))?;
        let packed = weights.as_cuda_slice::<u8>()?.slice(start..end);
        let p = self.plan;
        let stream = device.cuda_stream();
        // SAFETY: XqPlan bounds grid, dimensions, row bytes and output. Rust
        // validates complete packed rows and finite scales before upload.
        // One full warp writes each admitted output exactly once. Cudarc
        // tracks the stream events of all borrowed input/output slices.
        let mut output = unsafe { stream.alloc::<f32>(p.tokens as usize * p.rows as usize) }.w()?;
        let mut builder = func.builder();
        builder
            .arg(&input)
            .arg(&packed)
            .arg(&mut output)
            .arg(&p.tokens)
            .arg(&p.rows)
            .arg(&p.columns)
            .arg(&p.type_id)
            .arg(&p.row_bytes);
        unsafe {
            builder.launch(LaunchConfig {
                grid_dim: (p.grid_x, 1, 1),
                block_dim: (128, 1, 1),
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

#[cfg(test)]
mod tests;
