//! Main/SWA 512-channel typed quantized rows, with token-interleaved storage.
//!
//! Exact formats follow Apache-2.0 DeepInfer cache_common.py at revision
//! 9bd9cf243fc53ef9afbf8f04d391d989117d6ab9: FP8 E4M3/group32 packed UE8M0
//! (528 bytes/token), or FP4 E2M1/group16 E4M3 scales (288 bytes/token).
//! Deterministic tensor composition preserves quantization semantics; it is
//! not a fused SM90 kernel, paged allocator or model-wide admission permit.
//! Public constructors require actual CUDA. Private CPU entry is for tests.

use aster_power::error::{PowerError, Result};
use aster_power::inference::InferenceLimits;
use candle_core::{DType, Device, Tensor};
use tokio_util::sync::CancellationToken;

use super::quant::{decode_e2m1, decode_e4m3, encode_e2m1, encode_e4m3, rounded_packed_scales};
use super::{invalid, tensor_error, validate_finite, CudaLayerDevice};
use crate::model::MAX_CONTEXT_TOKENS;

pub const MAIN_KV_DIM: usize = 512;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MainKvFormat {
    Fp8Ue8m0,
    Fp4E4m3,
}

impl MainKvFormat {
    pub fn parse(value: &str) -> Result<Self> {
        match value {
            "fp8_ue8m0" => Ok(Self::Fp8Ue8m0),
            "fp4_e4m3" => Ok(Self::Fp4E4m3),
            _ => Err(invalid(
                "unsupported main KV format; supported set: fp8_ue8m0, fp4_e4m3",
            )),
        }
    }
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Fp8Ue8m0 => "fp8_ue8m0",
            Self::Fp4E4m3 => "fp4_e4m3",
        }
    }
    pub fn bytes_per_token(self) -> usize {
        match self {
            Self::Fp8Ue8m0 => 528,
            Self::Fp4E4m3 => 288,
        }
    }
    pub fn group_size(self) -> usize {
        match self {
            Self::Fp8Ue8m0 => 32,
            Self::Fp4E4m3 => 16,
        }
    }
    fn data_bytes(self) -> usize {
        match self {
            Self::Fp8Ue8m0 => 512,
            Self::Fp4E4m3 => 256,
        }
    }
    fn scale_bytes(self) -> usize {
        MAIN_KV_DIM / self.group_size()
    }
}

pub struct MainKvRows {
    data: MainKvRowsData,
}

impl MainKvRows {
    pub fn encode(
        device: &CudaLayerDevice,
        format: MainKvFormat,
        post_rope: &Tensor,
        cancellation: &CancellationToken,
    ) -> Result<Self> {
        if !post_rope.device().same_device(device.tensor_device()) {
            return Err(invalid(
                "main KV encoder differs from its bound CUDA device",
            ));
        }
        Ok(Self {
            data: encode_rows(format, post_rope, device.limits(), cancellation)?,
        })
    }
    pub fn from_records(
        device: &CudaLayerDevice,
        format: MainKvFormat,
        records: &Tensor,
    ) -> Result<Self> {
        if !records.device().same_device(device.tensor_device()) {
            return Err(invalid(
                "main KV records differ from their bound CUDA device",
            ));
        }
        Ok(Self {
            data: MainKvRowsData::new(format, records, device.limits())?,
        })
    }
    pub fn format(&self) -> MainKvFormat {
        self.data.format
    }
    pub fn row_count(&self) -> usize {
        self.data.rows
    }
    pub fn records(&self) -> &Tensor {
        &self.data.records
    }
    pub(crate) fn rows_data(&self) -> &MainKvRowsData {
        &self.data
    }
}

pub(crate) struct MainKvRowsData {
    format: MainKvFormat,
    records: Tensor,
    rows: usize,
}

impl MainKvRowsData {
    pub(crate) fn new(
        format: MainKvFormat,
        records: &Tensor,
        limits: &InferenceLimits,
    ) -> Result<Self> {
        limits.validate()?;
        let (rows, width) = records.dims2().map_err(tensor_error)?;
        if rows > MAX_CONTEXT_TOKENS
            || width != format.bytes_per_token()
            || records.dtype() != DType::U8
        {
            return Err(invalid("main KV rows require bounded token-interleaved U8 records matching the explicit format"));
        }
        if rows > 0 {
            limits.checked_elements(records.dims(), "main KV records")?;
        }
        checked_bytes(
            limits,
            rows,
            MAIN_KV_DIM * 16,
            0,
            "main KV snapshot/validation tensors",
        )?;
        let records = records.force_contiguous().map_err(tensor_error)?.detach();
        if rows > 0 {
            let scales = records
                .narrow(1, format.data_bytes(), format.scale_bytes())
                .map_err(tensor_error)?;
            let max_scale = match format {
                MainKvFormat::Fp8Ue8m0 => 254_u8,
                MainKvFormat::Fp4E4m3 => 126_u8,
            };
            let valid = scales
                .ge(1_u8)
                .and_then(|v| v.mul(&scales.le(max_scale)?))
                .and_then(|v| v.to_dtype(DType::U32))
                .and_then(|v| v.sum_all())
                .and_then(|v| v.to_scalar::<u32>())
                .map_err(tensor_error)?;
            if valid as usize != rows * format.scale_bytes() {
                return Err(invalid(
                    "main KV scales contain zero/padding, negative or nonfinite encodings",
                ));
            }
            if format == MainKvFormat::Fp8Ue8m0 {
                let data = records
                    .narrow(1, 0, format.data_bytes())
                    .map_err(tensor_error)?;
                let valid = data
                    .ne(127_u8)
                    .and_then(|v| v.mul(&data.ne(255_u8)?))
                    .and_then(|v| v.to_dtype(DType::U32))
                    .and_then(|v| v.sum_all())
                    .and_then(|v| v.to_scalar::<u32>())
                    .map_err(tensor_error)?;
                if valid as usize != rows * MAIN_KV_DIM {
                    return Err(invalid("main FP8 data contains an E4M3 NaN encoding"));
                }
            }
        }
        Ok(Self {
            format,
            records,
            rows,
        })
    }
    pub(crate) fn format(&self) -> MainKvFormat {
        self.format
    }
    pub(crate) fn row_count(&self) -> usize {
        self.rows
    }
    pub(crate) fn records(&self) -> &Tensor {
        &self.records
    }
    pub(crate) fn device(&self) -> &Device {
        self.records.device()
    }
    pub(crate) fn retained_bytes(&self) -> u64 {
        (self.rows * self.format.bytes_per_token()) as u64
    }
    pub(crate) fn decode_selected(
        &self,
        indices: &Tensor,
        limits: &InferenceLimits,
    ) -> Result<Tensor> {
        let count = indices.dims1().map_err(tensor_error)?;
        if indices.dtype() != DType::U32 || !indices.device().same_device(self.device()) {
            return Err(invalid("main KV gather requires same-device U32 indices"));
        }
        self.check_decode(count, limits)?;
        if count == 0 {
            return Tensor::zeros((0, MAIN_KV_DIM), DType::BF16, self.device())
                .map_err(tensor_error);
        }
        let valid = indices
            .lt(self.rows as u32)
            .and_then(|v| v.to_dtype(DType::U32))
            .and_then(|v| v.sum_all())
            .and_then(|v| v.to_scalar::<u32>())
            .map_err(tensor_error)?;
        if valid as usize != count {
            return Err(invalid(
                "main KV gather contains out-of-range/padded indices",
            ));
        }
        let records = self
            .records
            .index_select(indices, 0)
            .map_err(tensor_error)?;
        decode_records(self.format, &records, count)
    }
    pub(crate) fn decode_prefix(&self, count: usize, limits: &InferenceLimits) -> Result<Tensor> {
        if count > self.rows {
            return Err(invalid("main KV decode prefix exceeds its stored rows"));
        }
        self.check_decode(count, limits)?;
        let records = self.records.narrow(0, 0, count).map_err(tensor_error)?;
        decode_records(self.format, &records, count)
    }
    fn check_decode(&self, count: usize, limits: &InferenceLimits) -> Result<()> {
        if count > 0 {
            limits.checked_elements(&[count, MAIN_KV_DIM], "decoded main KV")?;
        }
        checked_bytes(
            limits,
            count,
            MAIN_KV_DIM * 64,
            self.retained_bytes(),
            "main KV decode live tensors",
        )
    }
}

fn encode_rows(
    format: MainKvFormat,
    input: &Tensor,
    limits: &InferenceLimits,
    cancellation: &CancellationToken,
) -> Result<MainKvRowsData> {
    check_cancel(cancellation)?;
    let (rows, dim) = input.dims2().map_err(tensor_error)?;
    if input.dtype() != DType::BF16 || dim != MAIN_KV_DIM || rows > MAX_CONTEXT_TOKENS {
        return Err(invalid(
            "main KV encoder requires BF16 [bounded tokens,512] post-norm/RoPE values",
        ));
    }
    if rows > 0 {
        limits.checked_elements(input.dims(), "main KV encoder input")?;
    }
    checked_bytes(
        limits,
        rows,
        MAIN_KV_DIM * 128,
        0,
        "main KV encoder live tensors",
    )?;
    if rows == 0 {
        let records = Tensor::zeros((0, format.bytes_per_token()), DType::U8, input.device())
            .map_err(tensor_error)?;
        return MainKvRowsData::new(format, &records, limits);
    }
    validate_finite(input, false, "main KV encoder input")?;
    let math = || -> candle_core::Result<Tensor> {
        let group_size = format.group_size();
        let groups_per_row = format.scale_bytes();
        let x = input
            .detach()
            .to_dtype(DType::F32)?
            .reshape((rows * groups_per_row, group_size))?;
        let amax = x.abs()?.max_keepdim(1)?;
        let (scales, inverse) = match format {
            MainKvFormat::Fp8Ue8m0 => {
                let target = amax.clamp(1e-4_f32, f32::MAX)?.affine(1. / 448., 0.)?;
                let (codes, scale) = rounded_packed_scales(&target, 254)?;
                (codes.to_dtype(DType::U8)?, scale.recip()?)
            }
            MainKvFormat::Fp4E4m3 => {
                let target = amax
                    .clamp(6_f32 * 2_f32.powi(-9), f32::MAX)?
                    .affine(1. / 6., 0.)?
                    .clamp(2_f32.powi(-9), 448_f32)?;
                let codes = encode_e4m3(&target)?;
                let scale = decode_e4m3(&codes)?;
                (codes, scale.recip()?)
            }
        };
        let normalized = x.broadcast_mul(&inverse)?.reshape((rows, MAIN_KV_DIM))?;
        let data = match format {
            MainKvFormat::Fp8Ue8m0 => encode_e4m3(&normalized)?,
            MainKvFormat::Fp4E4m3 => encode_e2m1(&normalized)?,
        };
        Tensor::cat(&[data, scales.reshape((rows, groups_per_row))?], 1)
    };
    let records = math().map_err(tensor_error)?;
    check_cancel(cancellation)?;
    let encoded = MainKvRowsData::new(format, &records, limits)?;
    check_cancel(cancellation)?;
    Ok(encoded)
}

fn decode_records(format: MainKvFormat, records: &Tensor, rows: usize) -> Result<Tensor> {
    if rows == 0 {
        return Tensor::zeros((0, MAIN_KV_DIM), DType::BF16, records.device())
            .map_err(tensor_error);
    }
    let math = || -> candle_core::Result<Tensor> {
        let data = records.narrow(1, 0, format.data_bytes())?;
        let scales = records.narrow(1, format.data_bytes(), format.scale_bytes())?;
        let values = match format {
            MainKvFormat::Fp8Ue8m0 => decode_e4m3(&data)?,
            MainKvFormat::Fp4E4m3 => decode_e2m1(&data)?,
        };
        let scales = match format {
            MainKvFormat::Fp8Ue8m0 => Tensor::from_vec(
                (0..255_u32)
                    .map(|code| f32::from_bits(code << 23))
                    .collect::<Vec<_>>(),
                255,
                records.device(),
            )?
            .index_select(&scales.flatten_all()?.to_dtype(DType::U32)?, 0)?
            .reshape((rows, format.scale_bytes(), 1))?,
            MainKvFormat::Fp4E4m3 => {
                decode_e4m3(&scales)?.reshape((rows, format.scale_bytes(), 1))?
            }
        };
        values
            .reshape((rows, format.scale_bytes(), format.group_size()))?
            .broadcast_mul(&scales)?
            .reshape((rows, MAIN_KV_DIM))?
            .to_dtype(DType::BF16)
    };
    let output = math().map_err(tensor_error)?.detach();
    validate_finite(&output, false, "decoded main KV values")?;
    Ok(output)
}

fn checked_bytes(
    limits: &InferenceLimits,
    rows: usize,
    per_row: usize,
    retained: u64,
    label: &str,
) -> Result<()> {
    // Conservative named-tensor estimates. CUDA allocator/BLAS workspace and
    // aggregation across model/layers require separate Power qualification.
    let bytes = (rows as u64)
        .checked_mul(per_row as u64)
        .and_then(|n| n.checked_add(retained))
        .and_then(|n| n.checked_add(4096))
        .ok_or_else(|| invalid("main KV tensor-byte estimate overflowed"))?;
    limits.checked_state_bytes(bytes, label)?;
    Ok(())
}

fn check_cancel(cancellation: &CancellationToken) -> Result<()> {
    if cancellation.is_cancelled() {
        return Err(PowerError::InferenceCancelled);
    }
    Ok(())
}

#[cfg(test)]
mod tests;
