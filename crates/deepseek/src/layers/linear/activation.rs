//! Row-major group-32 E4M3/UE8M0 activations. F32 SwiGLU outputs are cast
//! directly, without an intermediate BF16 rounding. Tensor control only.

use aster_power::error::Result;
use candle_core::{DType, Tensor};
use tokio_util::sync::CancellationToken;

use super::{check_cancel, decode_scales, invalid, tensor_error, validate_finite, CudaLayerDevice};
use crate::layers::quant::{decode_e4m3, encode_e4m3, rounded_packed_scales};

pub struct Fp8Activations {
    data: Tensor,
    scales: Tensor,
}

impl Fp8Activations {
    /// Split a packed group-column interval, preserving existing data and SF
    /// bytes. This does not cast/quantize again or copy payloads to the host.
    pub(crate) fn column_range(
        &self,
        first: usize,
        columns: usize,
        cancellation: &CancellationToken,
    ) -> Result<Self> {
        check_cancel(cancellation)?;
        if columns == 0
            || !first.is_multiple_of(32)
            || !columns.is_multiple_of(32)
            || first
                .checked_add(columns)
                .is_none_or(|end| end > self.data.dims()[1])
        {
            return Err(invalid(
                "packed FP8 column range requires complete in-bounds group-32 columns",
            ));
        }
        let data = self.data.narrow(1, first, columns).map_err(tensor_error)?;
        let scales = self
            .scales
            .narrow(1, first / 32, columns / 32)
            .map_err(tensor_error)?;
        check_cancel(cancellation)?;
        Ok(Self { data, scales })
    }

    pub fn encode(
        device: &CudaLayerDevice,
        input: &Tensor,
        cancellation: &CancellationToken,
    ) -> Result<Self> {
        check_cancel(cancellation)?;
        let (rows, columns) = input.dims2().map_err(tensor_error)?;
        if rows == 0
            || columns == 0
            || !columns.is_multiple_of(32)
            || !matches!(input.dtype(), DType::BF16 | DType::F32)
            || !input.device().same_device(device.tensor_device())
        {
            return Err(invalid("FP8 activations require same-device nonempty BF16/F32 [rows,complete group-32 columns]"));
        }
        device
            .limits()
            .checked_elements(input.dims(), "FP8 activation input")?;
        let bytes = (input.elem_count() as u64)
            .checked_mul(128)
            .and_then(|n| n.checked_add(4096))
            .ok_or_else(|| invalid("FP8 activation cast budget overflowed"))?;
        device
            .limits()
            .checked_state_bytes(bytes, "FP8 activation cast live tensors")?;
        validate_finite(input, false, "FP8 activation input")?;
        let cast = || -> candle_core::Result<(Tensor, Tensor)> {
            let groups = rows * columns / 32;
            let x = input.to_dtype(DType::F32)?.reshape((groups, 32))?;
            let target = x
                .abs()?
                .max_keepdim(1)?
                .clamp(1e-4_f32, f32::MAX)?
                .affine(1. / 448., 0.)?;
            let (codes, scales) = rounded_packed_scales(&target, 247)?;
            let data =
                encode_e4m3(&x.broadcast_mul(&scales.recip()?)?)?.reshape((rows, columns))?;
            Ok((
                data,
                codes.reshape((rows, columns / 32))?.to_dtype(DType::U8)?,
            ))
        };
        let (data, scales) = cast().map_err(tensor_error)?;
        check_cancel(cancellation)?;
        let encoded = Self::from_packed(device, &data, &scales)?;
        check_cancel(cancellation)?;
        Ok(encoded)
    }

    pub fn from_packed(device: &CudaLayerDevice, data: &Tensor, scales: &Tensor) -> Result<Self> {
        Self::snapshot_packed(device, data, scales, false)
    }

    /// Sparse checkpoint-table rows already carry UE8M0 exponents. Code zero
    /// has its OCP value 2^-127 here, rather than the live cast's padding tag.
    /// This boundary is private to the model's verified table gather.
    pub(crate) fn from_checkpoint_rows(
        device: &CudaLayerDevice,
        data: &Tensor,
        scales: &Tensor,
    ) -> Result<Self> {
        Self::snapshot_packed(device, data, scales, true)
    }

    fn snapshot_packed(
        device: &CudaLayerDevice,
        data: &Tensor,
        scales: &Tensor,
        checkpoint_scales: bool,
    ) -> Result<Self> {
        let (rows, columns) = data.dims2().map_err(tensor_error)?;
        if rows == 0
            || columns == 0
            || !columns.is_multiple_of(32)
            || data.dtype() != DType::U8
            || scales.dtype() != DType::U8
            || scales.dims() != [rows, columns / 32]
            || !data.device().same_device(device.tensor_device())
            || !scales.device().same_device(data.device())
        {
            return Err(invalid(
                "packed FP8 activations require same-device U8 values and group-32 exponent bytes",
            ));
        }
        device
            .limits()
            .checked_elements(data.dims(), "FP8 activation rows")?;
        device.limits().checked_state_bytes(
            (data.elem_count() as u64) * 16 + 4096,
            "FP8 activation validation",
        )?;
        let data = data.force_contiguous().map_err(tensor_error)?.detach();
        let scales = scales.force_contiguous().map_err(tensor_error)?.detach();
        super::validate_e4m3_bytes(&data)?;
        let minimum = scales
            .min_all()
            .and_then(|t| t.to_scalar::<u8>())
            .map_err(tensor_error)?;
        let maximum = scales
            .max_all()
            .and_then(|t| t.to_scalar::<u8>())
            .map_err(tensor_error)?;
        if maximum == 255 || (!checkpoint_scales && minimum == 0) {
            return Err(invalid(
                "packed FP8 scale is NaN or contains forbidden live-cast padding",
            ));
        }
        Ok(Self { data, scales })
    }
    pub fn data_bytes(&self) -> &Tensor {
        &self.data
    }
    pub fn scale_bytes(&self) -> &Tensor {
        &self.scales
    }

    /// Preserve packed bytes when dispatching rows to an expert. Construction
    /// snapshots/validates the format once; index_select owns each gathered
    /// section, so this does not requantize or alias a mutable input row.
    pub(crate) fn select_rows(
        &self,
        device: &CudaLayerDevice,
        ids: &Tensor,
        cancellation: &CancellationToken,
    ) -> Result<Self> {
        check_cancel(cancellation)?;
        let count = ids.dims1().map_err(tensor_error)?;
        let (rows, columns) = self.data.dims2().map_err(tensor_error)?;
        if count == 0
            || ids.dtype() != DType::U32
            || !ids.device().same_device(self.data.device())
            || !self.data.device().same_device(device.tensor_device())
        {
            return Err(invalid(
                "FP8 dispatch row IDs require nonempty same-device U32 data",
            ));
        }
        device
            .limits()
            .checked_elements(&[count, columns], "FP8 dispatch rows")?;
        let bytes = (count as u64)
            .checked_mul(columns as u64)
            .and_then(|n| n.checked_mul(16))
            .and_then(|n| n.checked_add(4096))
            .ok_or_else(|| invalid("FP8 dispatch tensor budget overflowed"))?;
        device
            .limits()
            .checked_state_bytes(bytes, "FP8 dispatch live tensors")?;
        let valid = ids
            .lt(rows as u32)
            .and_then(|t| t.to_dtype(DType::U32))
            .and_then(|t| t.sum_all())
            .and_then(|t| t.to_scalar::<u32>())
            .map_err(tensor_error)?;
        if valid as usize != count {
            return Err(invalid("FP8 dispatch row ID exceeds source rows"));
        }
        let data = self
            .data
            .index_select(ids, 0)
            .map_err(tensor_error)?
            .detach();
        let scales = self
            .scales
            .index_select(ids, 0)
            .map_err(tensor_error)?
            .detach();
        check_cancel(cancellation)?;
        Ok(Self { data, scales })
    }

    pub(crate) fn decode(&self) -> Result<Tensor> {
        let (rows, columns) = self.data.dims2().map_err(tensor_error)?;
        let scales = decode_scales(&self.scales)?;
        let output = decode_e4m3(&self.data)
            .and_then(|values| {
                values.mul(
                    &scales
                        .reshape((rows, columns / 32, 1))?
                        .broadcast_as((rows, columns / 32, 32))?
                        .reshape((rows, columns))?,
                )
            })
            .map_err(tensor_error)?;
        validate_finite(&output, false, "decoded FP8 activations")?;
        Ok(output)
    }
}
