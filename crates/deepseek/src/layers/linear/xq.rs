//! Packed XQ device matrices. The explicit F32 CUDA control never creates a
//! decoded resident matrix, changes quantization tiers or invokes HTTP.

use aster_power::error::{PowerError, Result};
use candle_core::{DType, Tensor};
use tokio_util::sync::CancellationToken;

use crate::cuda::xq::XqProjectionReceipt;
use crate::layers::{tensor_error, CudaLayerDevice};
use crate::weights::xq::experts::XqExpertProjection;
use crate::weights::xq::storage::XqWeights;
use crate::weights::xq::XqTensorType;

pub struct XqDeviceMatrix {
    device: CudaLayerDevice,
    kind: XqTensorType,
    shape: [usize; 2],
    packed: Tensor,
}

impl XqDeviceMatrix {
    pub(crate) fn device(&self) -> &CudaLayerDevice {
        &self.device
    }

    pub fn load(
        device: &CudaLayerDevice,
        weights: &XqWeights,
        tensor_name: &str,
        first_row: u64,
        rows: usize,
        cancellation: &CancellationToken,
    ) -> Result<Self> {
        check_cancel(cancellation)?;
        let tensor = weights.tensor(tensor_name)?;
        let kind = tensor.kind;
        if matches!(kind, XqTensorType::I8 | XqTensorType::I32) {
            return Err(invalid(
                "XQ integer metadata cannot be uploaded as a projection",
            ));
        }
        let columns = usize::try_from(tensor.shape[0])
            .map_err(|_| invalid("XQ matrix width exceeds usize"))?;
        let row_bytes = kind.row_bytes(columns as u64)?;
        let end = first_row
            .checked_add(rows as u64)
            .ok_or_else(|| invalid("XQ matrix source row overflowed"))?;
        if rows == 0 || end > tensor.bytes / row_bytes {
            return Err(invalid("XQ matrix upload exceeds its verified tensor"));
        }
        let retained = row_bytes
            .checked_mul(rows as u64)
            .ok_or_else(|| invalid("XQ matrix retained byte count overflowed"))?;
        if retained > device.limits().max_resident_weight_bytes {
            return Err(invalid("XQ matrix exceeds its resident weight budget"));
        }
        let row_bytes_usize =
            usize::try_from(row_bytes).map_err(|_| invalid("XQ packed row exceeds usize"))?;
        device
            .limits()
            .checked_elements(&[rows, row_bytes_usize], "XQ packed matrix")?;
        let chunk_rows = usize::try_from((weights.max_read_bytes() / row_bytes).min(rows as u64))
            .map_err(|_| invalid("XQ upload chunk rows exceed usize"))?;
        if chunk_rows == 0 {
            return Err(invalid("XQ read budget cannot fit one packed row"));
        }
        let chunks = rows.div_ceil(chunk_rows) as u64;
        let live = retained
            .checked_mul(2)
            .and_then(|bytes| bytes.checked_add(weights.max_read_bytes()))
            .and_then(|bytes| {
                chunks
                    .checked_mul(256)
                    .and_then(|meta| bytes.checked_add(meta))
            })
            .ok_or_else(|| invalid("XQ upload live byte count overflowed"))?;
        device
            .limits()
            .checked_state_bytes(live, "XQ packed upload and concatenation")?;
        let mut parts = Vec::new();
        parts
            .try_reserve_exact(rows.div_ceil(chunk_rows))
            .map_err(|_| invalid("XQ upload chunk list allocation failed"))?;
        let mut copied = 0;
        while copied < rows {
            check_cancel(cancellation)?;
            let count = chunk_rows.min(rows - copied);
            let raw = weights.read_rows(
                tensor_name,
                first_row + copied as u64,
                count as u64,
                cancellation,
            )?;
            kind.validate_floating_payload(raw.bytes(), columns as u64, count as u64)?;
            parts.push(
                Tensor::from_raw_buffer(
                    raw.bytes(),
                    DType::U8,
                    &[count, row_bytes_usize],
                    device.tensor_device(),
                )
                .map_err(tensor_error)?,
            );
            copied += count;
        }
        check_cancel(cancellation)?;
        let packed = if parts.len() == 1 {
            parts.remove(0)
        } else {
            Tensor::cat(&parts, 0).map_err(tensor_error)?
        };
        Ok(Self {
            device: device.clone(),
            kind,
            shape: [rows, columns],
            packed,
        })
    }

    pub fn load_expert(
        device: &CudaLayerDevice,
        weights: &XqWeights,
        projection: &XqExpertProjection,
        cancellation: &CancellationToken,
    ) -> Result<Self> {
        let rows = usize::try_from(projection.rows())
            .map_err(|_| invalid("XQ expert output rows exceed usize"))?;
        Self::load(
            device,
            weights,
            projection.tensor_name(),
            projection.first_row(),
            rows,
            cancellation,
        )
    }

    pub fn kind(&self) -> XqTensorType {
        self.kind
    }
    pub fn logical_shape(&self) -> [usize; 2] {
        self.shape
    }
    pub fn retained_weight_bytes(&self) -> u64 {
        self.packed.elem_count() as u64
    }

    /// Decode only selected packed rows to F32 on SM90. Duplicate/nonordered
    /// IDs preserve their input order; integer metadata is never a matrix.
    pub fn gather_f32_rows(
        &self,
        indices: &Tensor,
        cancellation: &CancellationToken,
    ) -> Result<(Tensor, XqProjectionReceipt)> {
        check_cancel(cancellation)?;
        #[cfg(feature = "cuda")]
        {
            let output = crate::cuda::xq::execution::gather(
                &self.device,
                self.kind,
                self.shape,
                &self.packed,
                indices,
            )?;
            check_cancel(cancellation)?;
            Ok(output)
        }
        #[cfg(not(feature = "cuda"))]
        {
            let _ = indices;
            Err(PowerError::BackendNotAvailable(
                "XQ gather requires the cuda feature and an owned SM90 device".into(),
            ))
        }
    }

    /// Explicit math control accepting finite F32 activations. Its warp
    /// reduction is a separate numeric profile from the official FP8 path.
    /// Whole-model XQ activation/rounding semantics remain separately gated.
    pub fn forward_f32_control(
        &self,
        input: &Tensor,
        cancellation: &CancellationToken,
    ) -> Result<(Tensor, XqProjectionReceipt)> {
        self.forward_rows_f32_control(input, 0, self.shape[0], cancellation)
    }

    /// Borrows contiguous rows from this allocated stack for one projection.
    /// The complete packed allocation remains owned and charged by `self`;
    /// this call does not create a separately accounted matrix or copy weights.
    pub fn forward_rows_f32_control(
        &self,
        input: &Tensor,
        first_row: usize,
        rows: usize,
        cancellation: &CancellationToken,
    ) -> Result<(Tensor, XqProjectionReceipt)> {
        check_cancel(cancellation)?;
        let packed = self.row_view(first_row, rows)?;
        #[cfg(feature = "cuda")]
        {
            let result = crate::cuda::xq::execution::project(
                &self.device,
                self.kind,
                [rows, self.shape[1]],
                &packed,
                input,
            )?;
            check_cancel(cancellation)?;
            Ok(result)
        }
        #[cfg(not(feature = "cuda"))]
        {
            let _ = (input, &self.device, packed);
            Err(PowerError::BackendNotAvailable(
                "XQ F32 control requires the cuda feature and an owned SM90 device".into(),
            ))
        }
    }

    fn row_view(&self, first: usize, rows: usize) -> Result<Tensor> {
        let end = first
            .checked_add(rows)
            .ok_or_else(|| invalid("XQ matrix row view overflowed"))?;
        if rows == 0 || end > self.shape[0] {
            return Err(invalid(
                "XQ matrix row view is empty or exceeds its allocated stack",
            ));
        }
        self.packed.narrow(0, first, rows).map_err(tensor_error)
    }

    #[cfg(test)]
    pub(crate) fn from_bytes_for_tests(
        device: &CudaLayerDevice,
        kind: XqTensorType,
        rows: usize,
        columns: usize,
        raw: &[u8],
    ) -> Result<Self> {
        kind.validate_floating_payload(raw, columns as u64, rows as u64)?;
        let row_bytes = kind.row_bytes(columns as u64)? as usize;
        let packed =
            Tensor::from_raw_buffer(raw, DType::U8, &[rows, row_bytes], device.tensor_device())
                .map_err(tensor_error)?;
        Ok(Self {
            device: device.clone(),
            kind,
            shape: [rows, columns],
            packed,
        })
    }
}

fn check_cancel(cancellation: &CancellationToken) -> Result<()> {
    if cancellation.is_cancelled() {
        return Err(PowerError::InferenceCancelled);
    }
    Ok(())
}

fn invalid(reason: &str) -> PowerError {
    PowerError::InvalidRequest(reason.into())
}

#[cfg(test)]
mod tests;
