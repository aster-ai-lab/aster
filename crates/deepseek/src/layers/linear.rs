//! Model-owned raw device matrices and bounded tensor linear control.
//! Upstream BF16/F32/FP8/MXFP4 bytes retain their declared layouts. Quantized
//! linear requires actual packed FP8 activations, never an implicit BF16 path.
//! CUDA dispatch selects SM90 group-32 MMA explicitly; an independently named
//! F32 tiled control remains available. Full model resource reservation and
//! numerical/performance promotion remain separate qualification obligations.

use aster_power::error::{PowerError, Result};
use candle_core::{DType, Tensor};
use tokio_util::sync::CancellationToken;

use super::quant::{decode_e2m1, decode_e4m3};
use super::{invalid, tensor_error, validate_finite, CudaLayerDevice};
use crate::weights::{MatrixLayout, MatrixWeights};

pub mod activation;
pub mod expert;
mod upload;
pub mod xq;
use activation::Fp8Activations;

const OUTPUT_TILE: usize = 32;

pub struct DeviceMatrix {
    device: CudaLayerDevice,
    layout: MatrixLayout,
    shape: [usize; 2],
    data: Tensor,
    scales: Option<Tensor>,
    scale_row_map: Option<Tensor>,
    source_first_row: usize,
    source_total_rows: usize,
}

impl DeviceMatrix {
    pub(crate) fn device(&self) -> &CudaLayerDevice {
        &self.device
    }

    #[cfg(test)]
    pub(crate) fn from_tensors_for_tests(
        device: &CudaLayerDevice,
        layout: MatrixLayout,
        shape: [usize; 2],
        data: &Tensor,
        scales: Option<&Tensor>,
    ) -> Result<Self> {
        Self::from_tensors(device, layout, shape, data, scales)
    }
    /// Read through the verified Power collection, preserving its range/hash,
    /// zeroizing ownership and cancellation boundary before device upload.
    pub fn load(
        device: &CudaLayerDevice,
        weights: &MatrixWeights<'_>,
        first_row: usize,
        rows: usize,
        cancellation: &CancellationToken,
    ) -> Result<Self> {
        check_cancel(cancellation)?;
        let descriptor = weights.descriptor();
        let columns = descriptor.logical_shape()[1];
        check_capacity(device, [rows, columns], descriptor.layout())?;
        let layout = descriptor.layout();
        let (data, scales) = upload::read_chunks(device, weights, first_row, rows, cancellation)?;
        check_cancel(cancellation)?;
        let mut loaded =
            Self::from_tensors(device, layout, [rows, columns], &data, scales.as_ref())?;
        loaded.source_first_row = first_row;
        loaded.source_total_rows = descriptor.logical_shape()[0];
        check_cancel(cancellation)?;
        Ok(loaded)
    }

    fn from_tensors(
        device: &CudaLayerDevice,
        layout: MatrixLayout,
        shape: [usize; 2],
        data: &Tensor,
        scales: Option<&Tensor>,
    ) -> Result<Self> {
        check_capacity(device, shape, layout)?;
        let [rows, columns] = shape;
        let (dtype, physical_columns) = match layout {
            MatrixLayout::Bf16 => (DType::BF16, columns),
            MatrixLayout::F32 => (DType::F32, columns),
            MatrixLayout::Mxfp4Block32 => (DType::U8, columns / 2),
            _ => (DType::U8, columns),
        };
        if data.dtype() != dtype
            || data.dims() != [rows, physical_columns]
            || !data.device().same_device(device.tensor_device())
        {
            return Err(invalid(
                "device matrix bytes/dtype/device differ from its explicit layout",
            ));
        }
        let quantized = !matches!(layout, MatrixLayout::Bf16 | MatrixLayout::F32);
        if scales.is_some() != quantized {
            return Err(invalid(
                "device matrix scales are missing or forbidden for its layout",
            ));
        }
        if let Some(scales) = scales {
            let scale_rows = if layout == MatrixLayout::Fp8Block32 {
                rows.div_ceil(32)
            } else {
                rows
            };
            if scales.dtype() != DType::U8
                || scales.dims() != [scale_rows, columns.div_ceil(32)]
                || !scales.device().same_device(data.device())
            {
                return Err(invalid(
                    "device matrix exponent shape/dtype/device differs from its scale layout",
                ));
            }
            if scales
                .max_all()
                .and_then(|t| t.to_scalar::<u8>())
                .map_err(tensor_error)?
                == 255
            {
                return Err(invalid("device matrix contains reserved UE8M0 NaN scale"));
            }
        }
        let data = data.force_contiguous().map_err(tensor_error)?.detach();
        let scales = scales
            .map(|t| {
                t.force_contiguous()
                    .map(|t| t.detach())
                    .map_err(tensor_error)
            })
            .transpose()?;
        match layout {
            MatrixLayout::Bf16 | MatrixLayout::F32 => {
                validate_finite(&data, false, "device matrix")?
            }
            MatrixLayout::Mxfp4Block32 => {}
            _ => validate_e4m3_bytes(&data)?,
        }
        let scale_row_map = if layout == MatrixLayout::Fp8Block32 {
            Some(
                Tensor::from_vec(
                    (0..rows).map(|r| (r / 32) as u32).collect::<Vec<_>>(),
                    rows,
                    data.device(),
                )
                .map_err(tensor_error)?,
            )
        } else {
            None
        };
        Ok(Self {
            device: device.clone(),
            layout,
            shape,
            data,
            scales,
            scale_row_map,
            source_first_row: 0,
            source_total_rows: rows,
        })
    }

    pub fn layout(&self) -> MatrixLayout {
        self.layout
    }
    pub fn logical_shape(&self) -> [usize; 2] {
        self.shape
    }
    pub fn source_first_row(&self) -> usize {
        self.source_first_row
    }
    pub fn source_total_rows(&self) -> usize {
        self.source_total_rows
    }
    pub fn retained_weight_bytes(&self) -> u64 {
        let data = self.data.elem_count() as u64 * self.data.dtype().size_in_bytes() as u64;
        data + self.scales.as_ref().map_or(0, |t| t.elem_count() as u64)
            + self
                .scale_row_map
                .as_ref()
                .map_or(0, |t| t.elem_count() as u64 * 4)
    }

    /// A complete FP8 output-group range. Scale rows are rebased without
    /// decoding/requantizing values. Construction snapshots the raw sections;
    /// callers must include the temporary source plus copies in their budget.
    pub(crate) fn fp8_output_rows(
        &self,
        first: usize,
        rows: usize,
        cancellation: &CancellationToken,
    ) -> Result<Self> {
        check_cancel(cancellation)?;
        if self.layout != MatrixLayout::Fp8Block32
            || rows == 0
            || !first.is_multiple_of(32)
            || !rows.is_multiple_of(32)
            || first
                .checked_add(rows)
                .is_none_or(|end| end > self.shape[0])
        {
            return Err(invalid(
                "FP8 output groups require complete aligned block-32 row ranges",
            ));
        }
        let data = self.data.narrow(0, first, rows).map_err(tensor_error)?;
        let scales = self
            .scales
            .as_ref()
            .ok_or_else(|| invalid("FP8 output group is missing its scale rows"))?
            .narrow(0, first / 32, rows / 32)
            .map_err(tensor_error)?;
        let mut group = Self::from_tensors(
            &self.device,
            self.layout,
            [rows, self.shape[1]],
            &data,
            Some(&scales),
        )?;
        group.source_first_row = self
            .source_first_row
            .checked_add(first)
            .ok_or_else(|| invalid("FP8 group source offset overflowed"))?;
        group.source_total_rows = self.source_total_rows;
        check_cancel(cancellation)?;
        Ok(group)
    }

    /// Gather IDs relative to the uploaded row chunk, including repeated IDs. This can
    /// supply token/Engram embeddings without a full-table F32 expansion.
    pub fn gather_rows(
        &self,
        ids: &Tensor,
        output: DType,
        cancellation: &CancellationToken,
    ) -> Result<Tensor> {
        check_cancel(cancellation)?;
        validate_output(output)?;
        let count = ids.dims1().map_err(tensor_error)?;
        if count == 0 || ids.dtype() != DType::U32 || !ids.device().same_device(self.data.device())
        {
            return Err(invalid(
                "matrix row gather requires nonempty same-device U32 IDs",
            ));
        }
        self.device
            .limits()
            .checked_elements(&[count, self.shape[1]], "matrix gathered rows")?;
        let width = if self.scales.is_some() {
            self.shape[1].div_ceil(32) * 32
        } else {
            self.shape[1]
        };
        let bytes = (count as u64)
            .checked_mul(width as u64)
            .and_then(|n| n.checked_mul(48))
            .ok_or_else(|| invalid("matrix gather budget overflowed"))?;
        self.device
            .limits()
            .checked_state_bytes(bytes, "matrix gather working tensors")?;
        let valid = ids
            .lt(self.shape[0] as u32)
            .and_then(|t| t.to_dtype(DType::U32))
            .and_then(|t| t.sum_all())
            .and_then(|t| t.to_scalar::<u32>())
            .map_err(tensor_error)?;
        if valid as usize != count {
            return Err(invalid("matrix row gather contains an invalid logical ID"));
        }
        let values = self.decode_rows(ids)?;
        let output = values.to_dtype(output).map_err(tensor_error)?.detach();
        validate_finite(&output, false, "matrix gathered output")?;
        check_cancel(cancellation)?;
        Ok(output)
    }

    fn decode_rows(&self, ids: &Tensor) -> Result<Tensor> {
        let count = ids.elem_count();
        let columns = self.shape[1];
        let width = if self.scales.is_some() {
            columns.div_ceil(32) * 32
        } else {
            columns
        };
        self.device
            .limits()
            .checked_elements(&[count, width], "matrix decoded scale/value rows")?;
        let data = self.data.index_select(ids, 0).map_err(tensor_error)?;
        let values = match self.layout {
            MatrixLayout::Bf16 | MatrixLayout::F32 => data.to_dtype(DType::F32),
            MatrixLayout::Mxfp4Block32 => decode_e2m1(&data),
            _ => decode_e4m3(&data),
        }
        .map_err(tensor_error)?;
        let values = if let Some(scales) = &self.scales {
            let row_ids = match &self.scale_row_map {
                Some(map) => map.index_select(ids, 0).map_err(tensor_error)?,
                None => ids.clone(),
            };
            let scales = decode_scales(&scales.index_select(&row_ids, 0).map_err(tensor_error)?)?;
            let groups = columns.div_ceil(32);
            values
                .mul(
                    &scales
                        .reshape((count, groups, 1))
                        .and_then(|t| t.broadcast_as((count, groups, 32)))
                        .and_then(|t| t.reshape((count, groups * 32)))
                        .and_then(|t| t.narrow(1, 0, columns))
                        .map_err(tensor_error)?,
                )
                .map_err(tensor_error)?
        } else {
            values
        };
        validate_finite(&values, false, "decoded matrix rows")?;
        Ok(values)
    }

    pub fn forward_bf16(
        &self,
        input: &Tensor,
        output: DType,
        cancellation: &CancellationToken,
    ) -> Result<Tensor> {
        if self.layout != MatrixLayout::Bf16 || input.dtype() != DType::BF16 {
            return Err(invalid(
                "BF16 projection requires BF16 parameters and BF16 operands",
            ));
        }
        self.forward_decoded(input, output, cancellation)
    }
    pub fn forward_f32(
        &self,
        input: &Tensor,
        output: DType,
        cancellation: &CancellationToken,
    ) -> Result<Tensor> {
        if self.layout != MatrixLayout::F32 || input.dtype() != DType::F32 {
            return Err(invalid(
                "FP32 projection requires FP32 parameters and FP32 operands",
            ));
        }
        self.forward_decoded(input, output, cancellation)
    }
    pub fn forward_fp8(
        &self,
        input: &Fp8Activations,
        output: DType,
        cancellation: &CancellationToken,
    ) -> Result<Tensor> {
        self.forward_fp8_with_profile(
            input,
            output,
            self.device.projection_profile(),
            cancellation,
        )
        .map(|(tensor, _)| tensor)
    }

    pub fn forward_fp8_with_profile(
        &self,
        input: &Fp8Activations,
        output: DType,
        profile: crate::cuda::ProjectionProfile,
        cancellation: &CancellationToken,
    ) -> Result<(Tensor, crate::cuda::ProjectionReceipt)> {
        use crate::cuda::{ProjectionProfile, ProjectionReceipt};
        self.validate_fp8_forward(input, output, cancellation)?;
        match profile {
            ProjectionProfile::TensorControlV1 => {
                use sha2::{Digest, Sha256};
                let tensor = self.forward_fp8_control(input, output, cancellation)?;
                let receipt = ProjectionReceipt {
                    profile,
                    abi: "aster-f32-tiled-control-v1",
                    source_sha256: format!("{:x}", Sha256::digest(include_bytes!("linear.rs"))),
                    ptx_sha256: None,
                    compile_options: "Candle F32 tiled control; no custom PTX",
                    kernel_attributes: None,
                    scratch_bytes: self.live_tensor_bytes(input.data_bytes().dims()[0])?,
                    kernel_output_bytes: tensor.elem_count() as u64 * 4,
                    output_bytes: tensor.elem_count() as u64 * output.size_in_bytes() as u64,
                };
                Ok((tensor, receipt))
            }
            ProjectionProfile::Sm90Group32MmaV1 | ProjectionProfile::Sm90ScalarF32V1 => {
                #[cfg(feature = "cuda")]
                {
                    let scales = self
                        .scales
                        .as_ref()
                        .ok_or_else(|| invalid("SM90 projection is missing scale bytes"))?;
                    let (tensor, mut receipt) = crate::cuda::execution::project(
                        &self.device,
                        profile,
                        self.layout,
                        input.data_bytes(),
                        input.scale_bytes(),
                        &self.data,
                        scales,
                        self.shape[0],
                    )?;
                    validate_finite(&tensor, false, "SM90 raw projection accumulation")?;
                    let tensor = tensor.to_dtype(output).map_err(tensor_error)?.detach();
                    validate_finite(&tensor, false, "SM90 raw projection output")?;
                    receipt.output_bytes =
                        tensor.elem_count() as u64 * output.size_in_bytes() as u64;
                    check_cancel(cancellation)?;
                    Ok((tensor, receipt))
                }
                #[cfg(not(feature = "cuda"))]
                Err(PowerError::BackendNotAvailable(
                    "SM90 projection requires the cuda feature; no implicit fallback".into(),
                ))
            }
        }
    }

    fn validate_fp8_forward(
        &self,
        input: &Fp8Activations,
        output: DType,
        cancellation: &CancellationToken,
    ) -> Result<()> {
        check_cancel(cancellation)?;
        if !matches!(
            self.layout,
            MatrixLayout::Fp8Block32 | MatrixLayout::Mxfp4Block32
        ) || !input.data_bytes().device().same_device(self.data.device())
        {
            return Err(invalid("quantized projection requires dense FP8/MXFP4 weights and matching packed FP8 activation device"));
        }
        self.check_forward(input.data_bytes(), output)
    }

    fn forward_fp8_control(
        &self,
        input: &Fp8Activations,
        output: DType,
        cancellation: &CancellationToken,
    ) -> Result<Tensor> {
        self.validate_fp8_forward(input, output, cancellation)?;
        self.forward_decoded(&input.decode()?, output, cancellation)
    }

    fn check_forward(&self, input: &Tensor, output: DType) -> Result<()> {
        validate_output(output)?;
        let (tokens, columns) = input.dims2().map_err(tensor_error)?;
        if tokens == 0
            || columns != self.shape[1]
            || !input.device().same_device(self.data.device())
        {
            return Err(invalid("linear input differs from matrix geometry/device"));
        }
        self.live_tensor_bytes(tokens)?;
        Ok(())
    }

    pub(crate) fn live_tensor_bytes(&self, tokens: usize) -> Result<u64> {
        let columns = self.shape[1];
        let limits = self.device.limits();
        let input_elements = limits.checked_elements(&[tokens, columns], "linear input")? as u64;
        let output_elements =
            limits.checked_elements(&[tokens, self.shape[0]], "linear output")? as u64;
        let width = if self.scales.is_some() {
            columns.div_ceil(32) * 32
        } else {
            columns
        };
        let tile_elements = limits.checked_elements(
            &[self.shape[0].min(OUTPUT_TILE), width],
            "linear decoded weight tile",
        )? as u64;
        let bytes = input_elements
            .checked_mul(24)
            .and_then(|n| {
                output_elements
                    .checked_mul(24)
                    .and_then(|m| n.checked_add(m))
            })
            .and_then(|n| tile_elements.checked_mul(48).and_then(|m| n.checked_add(m)))
            .and_then(|n| n.checked_add(4096))
            .ok_or_else(|| invalid("linear tensor budget overflowed"))?;
        limits.checked_state_bytes(bytes, "linear tiled control live tensors")
    }
    fn forward_decoded(
        &self,
        input: &Tensor,
        output: DType,
        cancellation: &CancellationToken,
    ) -> Result<Tensor> {
        check_cancel(cancellation)?;
        self.check_forward(input, output)?;
        validate_finite(input, false, "linear input")?;
        let input = input.to_dtype(DType::F32).map_err(tensor_error)?;
        let mut chunks = Vec::with_capacity(self.shape[0].div_ceil(OUTPUT_TILE));
        for first in (0..self.shape[0]).step_by(OUTPUT_TILE) {
            check_cancel(cancellation)?;
            let end = (first + OUTPUT_TILE).min(self.shape[0]);
            let ids =
                Tensor::arange(first as u32, end as u32, input.device()).map_err(tensor_error)?;
            let weight = self.decode_rows(&ids)?;
            let part = input
                .matmul(
                    &weight
                        .t()
                        .and_then(|t| t.contiguous())
                        .map_err(tensor_error)?,
                )
                .map_err(tensor_error)?;
            validate_finite(&part, false, "linear accumulation")?;
            chunks.push(part);
        }
        let result = Tensor::cat(&chunks, 1)
            .and_then(|t| t.to_dtype(output))
            .map_err(tensor_error)?
            .detach();
        validate_finite(&result, false, "linear output")?;
        check_cancel(cancellation)?;
        Ok(result)
    }
}

fn check_capacity(device: &CudaLayerDevice, shape: [usize; 2], layout: MatrixLayout) -> Result<()> {
    let [rows, columns] = shape;
    device.limits().checked_elements(&shape, "device matrix")?;
    if rows >= u32::MAX as usize
        || (matches!(layout, MatrixLayout::Mxfp4Block32 | MatrixLayout::Fp8Row32)
            && !columns.is_multiple_of(32))
    {
        return Err(invalid(
            "device matrix row IDs or quantization groups exceed supported bounds",
        ));
    }
    let elements = (rows as u64)
        .checked_mul(columns as u64)
        .ok_or_else(|| invalid("device matrix element count overflowed"))?;
    let bytes = match layout {
        MatrixLayout::Bf16 => elements.checked_mul(2),
        MatrixLayout::F32 => elements.checked_mul(4),
        MatrixLayout::Fp8Block32 => (rows.div_ceil(32) as u64)
            .checked_mul(columns.div_ceil(32) as u64)
            .and_then(|n| n.checked_add(elements))
            .and_then(|n| (rows as u64).checked_mul(4).and_then(|m| n.checked_add(m))),
        MatrixLayout::Fp8Row32 => (rows as u64)
            .checked_mul((columns / 32) as u64)
            .and_then(|n| n.checked_add(elements)),
        MatrixLayout::Mxfp4Block32 => elements.checked_div(2).and_then(|n| {
            (rows as u64)
                .checked_mul((columns / 32) as u64)
                .and_then(|m| n.checked_add(m))
        }),
    }
    .ok_or_else(|| invalid("device matrix upload budget overflowed"))?;
    if bytes > device.limits().max_resident_weight_bytes {
        return Err(invalid(
            "device matrix exceeds its resident-weight byte budget",
        ));
    }
    device.limits().checked_state_bytes(
        bytes
            .checked_mul(16)
            .ok_or_else(|| invalid("device matrix validation budget overflowed"))?,
        "device matrix upload/validation tensors",
    )?;
    Ok(())
}
fn validate_output(dtype: DType) -> Result<()> {
    if matches!(dtype, DType::F32 | DType::BF16) {
        Ok(())
    } else {
        Err(invalid("linear/gather output must be FP32 or BF16"))
    }
}
fn validate_e4m3_bytes(data: &Tensor) -> Result<()> {
    let count = u32::try_from(data.elem_count())
        .map_err(|_| invalid("FP8 validation count exceeds u32"))?;
    let valid = data
        .ne(127_u8)
        .and_then(|t| t.mul(&data.ne(255_u8)?))
        .and_then(|t| t.to_dtype(DType::U32))
        .and_then(|t| t.sum_all())
        .and_then(|t| t.to_scalar::<u32>())
        .map_err(tensor_error)?;
    if valid != count {
        return Err(invalid("FP8 data contains a reserved E4M3 NaN code"));
    }
    Ok(())
}
fn decode_scales(scales: &Tensor) -> Result<Tensor> {
    // General checkpoint UE8M0 byte 0 is 2^-127, unlike live packed padding.
    let lut: Vec<_> = (0..=255_u32)
        .map(|code| {
            if code == 255 {
                f32::NAN
            } else {
                f32::from_bits(if code == 0 { 1 << 22 } else { code << 23 })
            }
        })
        .collect();
    Tensor::from_vec(lut, 256, scales.device())
        .and_then(|t| t.index_select(&scales.flatten_all()?.to_dtype(DType::U32)?, 0))
        .and_then(|t| t.reshape(scales.shape()))
        .map_err(tensor_error)
}
fn check_cancel(cancellation: &CancellationToken) -> Result<()> {
    if cancellation.is_cancelled() {
        Err(PowerError::InferenceCancelled)
    } else {
        Ok(())
    }
}

#[cfg(test)]
mod tests;
