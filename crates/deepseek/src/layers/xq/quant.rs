//! XQ's explicit F32 activation quantization profile from pinned
//! ds41_ops.hpp. Its division boundaries differ from the official BF16
//! codec's reciprocal products. Encoded bytes reuse existing typed storage;
//! they do not remove the quantization or implicitly round its input to BF16.

use aster_power::error::Result;
use candle_core::{DType, Tensor};
use tokio_util::sync::CancellationToken;

use super::{check_cancel, invalid, same_device};
use crate::{
    layers::{
        kv::{MainKvFormat, MainKvRows},
        quant::{
            decode_e2m1, decode_e4m3, encode_e2m1, encode_e4m3, rounded_packed_scales, Fp4IndexRows,
        },
        tensor_error, validate_finite, CudaLayerDevice,
    },
    model::MAX_CONTEXT_TOKENS,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum XqActivationFormat {
    WindowFp8,
    CompressedFp4,
    IndexFp4,
}

impl XqActivationFormat {
    pub fn parse(value: &str) -> Result<Self> {
        match value {
            "fp8_pow2_32" => Ok(Self::WindowFp8),
            "fp4_e4m3_16" => Ok(Self::CompressedFp4),
            "fp4_pow2_32" => Ok(Self::IndexFp4),
            _ => Err(invalid("unsupported XQ activation format; supported set: fp8_pow2_32, fp4_e4m3_16, fp4_pow2_32")),
        }
    }
    pub fn as_str(self) -> &'static str {
        match self {
            Self::WindowFp8 => "fp8_pow2_32",
            Self::CompressedFp4 => "fp4_e4m3_16",
            Self::IndexFp4 => "fp4_pow2_32",
        }
    }
    fn columns(self) -> usize {
        if self == Self::IndexFp4 {
            128
        } else {
            512
        }
    }
    fn group(self) -> usize {
        if self == Self::CompressedFp4 {
            16
        } else {
            32
        }
    }
    fn data_columns(self) -> usize {
        self.columns() / if self == Self::WindowFp8 { 1 } else { 2 }
    }
    fn max_value(self) -> f32 {
        if self == Self::WindowFp8 {
            448.
        } else {
            6.
        }
    }
    fn amax_floor(self) -> f32 {
        match self {
            Self::WindowFp8 => 1e-4,
            Self::CompressedFp4 => 6. * 2_f32.powi(-9),
            Self::IndexFp4 => 6. * f32::MIN_POSITIVE,
        }
    }
}

/// Immutable original packed data/scales with explicit XQ F32 provenance.
/// These are correctness-control tensor compositions, not fused cast kernels.
pub struct XqQuantizedActivation {
    device: CudaLayerDevice,
    format: XqActivationFormat,
    rows: usize,
    data: Tensor,
    scales: Tensor,
}

impl XqQuantizedActivation {
    pub fn encode(
        device: &CudaLayerDevice,
        format: XqActivationFormat,
        input: &Tensor,
        cancellation: &CancellationToken,
    ) -> Result<Self> {
        check_cancel(cancellation)?;
        same_device(device, input)?;
        device.limits().validate()?;
        let (rows, columns) = input.dims2().map_err(tensor_error)?;
        let max_rows = device.limits().max_context_tokens.min(MAX_CONTEXT_TOKENS)
            * if format == XqActivationFormat::IndexFp4 {
                32
            } else {
                1
            };
        if input.dtype() != DType::F32 || columns != format.columns() || rows > max_rows {
            return Err(invalid("XQ activation codec requires bounded same-device F32 rows of width 512 (KV) or 128 (index)"));
        }
        if rows > 0 {
            device
                .limits()
                .checked_elements(input.dims(), "XQ F32 activation rows")?;
        }
        live_bytes(device, format, rows, "XQ activation encoder live tensors")?;
        let groups_per_row = columns / format.group();
        let (data, scales) = if rows == 0 {
            (
                Tensor::zeros((0, format.data_columns()), DType::U8, input.device())
                    .map_err(tensor_error)?,
                Tensor::zeros((0, groups_per_row), DType::U8, input.device())
                    .map_err(tensor_error)?,
            )
        } else {
            validate_finite(input, false, "XQ activation encoder input")?;
            let x = input
                .detach()
                .reshape((rows * groups_per_row, format.group()))
                .map_err(tensor_error)?;
            let amax = x
                .abs()
                .and_then(|x| x.max_keepdim(1))
                .and_then(|x| x.clamp(format.amax_floor(), f32::MAX))
                .map_err(tensor_error)?;
            // Keep actual F32 division. A reciprocal product can flip an
            // E4M3/FP4 midpoint and is a different activation profile.
            let denominator =
                Tensor::new(format.max_value(), input.device()).map_err(tensor_error)?;
            let target = amax.broadcast_div(&denominator).map_err(tensor_error)?;
            let (scale_codes, scale) = if format == XqActivationFormat::CompressedFp4 {
                let count = target
                    .le(448_f32)
                    .and_then(|x| x.to_dtype(DType::U32))
                    .and_then(|x| x.sum_all())
                    .and_then(|x| x.to_scalar::<u32>())
                    .map_err(tensor_error)?;
                if count as usize != rows * groups_per_row {
                    return Err(invalid(
                        "XQ compressed E4M3 scales exceed their finite encoded range",
                    ));
                }
                let codes = encode_e4m3(&target).map_err(tensor_error)?;
                let scale = decode_e4m3(&codes).map_err(tensor_error)?;
                (codes, scale)
            } else {
                let maximum = if format == XqActivationFormat::IndexFp4 {
                    253
                } else {
                    254
                };
                let (codes, scale) =
                    rounded_packed_scales(&target, maximum).map_err(tensor_error)?;
                (codes.to_dtype(DType::U8).map_err(tensor_error)?, scale)
            };
            let normalized = x
                .broadcast_div(&scale)
                .and_then(|x| x.clamp(-format.max_value(), format.max_value()))
                .and_then(|x| x.reshape((rows, columns)))
                .map_err(tensor_error)?;
            let data = if format == XqActivationFormat::WindowFp8 {
                encode_e4m3(&normalized)
            } else {
                encode_e2m1(&normalized)
            }
            .map_err(tensor_error)?;
            (
                data,
                scale_codes
                    .reshape((rows, groups_per_row))
                    .map_err(tensor_error)?,
            )
        };
        let output = Self {
            device: device.clone(),
            format,
            rows,
            data,
            scales,
        };
        // A finite input can round onto an overflowing decoded grid at the
        // upper exponent boundary. Refuse before exposing packed storage.
        output.decode_f32(cancellation)?;
        check_cancel(cancellation)?;
        Ok(output)
    }

    pub fn format(&self) -> XqActivationFormat {
        self.format
    }
    pub fn row_count(&self) -> usize {
        self.rows
    }
    pub fn data_bytes(&self) -> &Tensor {
        &self.data
    }
    pub fn scale_bytes(&self) -> &Tensor {
        &self.scales
    }
    pub fn retained_bytes(&self) -> u64 {
        (self.data.elem_count() + self.scales.elem_count()) as u64
    }

    pub fn decode_f32(&self, cancellation: &CancellationToken) -> Result<Tensor> {
        check_cancel(cancellation)?;
        live_bytes(
            &self.device,
            self.format,
            self.rows,
            "XQ activation decoder live tensors",
        )?;
        if self.rows == 0 {
            return Tensor::zeros(
                (0, self.format.columns()),
                DType::F32,
                self.device.tensor_device(),
            )
            .map_err(tensor_error);
        }
        let groups = self.format.columns() / self.format.group();
        let values = if self.format == XqActivationFormat::WindowFp8 {
            decode_e4m3(&self.data)
        } else {
            decode_e2m1(&self.data)
        }
        .map_err(tensor_error)?;
        let scales = if self.format == XqActivationFormat::CompressedFp4 {
            decode_e4m3(&self.scales).map_err(tensor_error)?
        } else {
            Tensor::from_vec(
                (0..255_u32)
                    .map(|code| f32::from_bits(code << 23))
                    .collect::<Vec<_>>(),
                255,
                self.device.tensor_device(),
            )
            .and_then(|lut| lut.index_select(&self.scales.flatten_all()?.to_dtype(DType::U32)?, 0))
            .and_then(|x| x.reshape((self.rows, groups)))
            .map_err(tensor_error)?
        };
        let output = values
            .reshape((self.rows, groups, self.format.group()))
            .and_then(|x| x.broadcast_mul(&scales.unsqueeze(2)?))
            .and_then(|x| x.reshape((self.rows, self.format.columns())))
            .map_err(tensor_error)?
            .detach();
        validate_finite(&output, false, "XQ decoded activation grid")?;
        check_cancel(cancellation)?;
        Ok(output)
    }

    pub fn into_main_rows(self, cancellation: &CancellationToken) -> Result<MainKvRows> {
        check_cancel(cancellation)?;
        let format = match self.format {
            XqActivationFormat::WindowFp8 => MainKvFormat::Fp8Ue8m0,
            XqActivationFormat::CompressedFp4 => MainKvFormat::Fp4E4m3,
            XqActivationFormat::IndexFp4 => {
                return Err(invalid(
                    "index XQ activation cannot become main/window KV storage",
                ))
            }
        };
        live_bytes(
            &self.device,
            self.format,
            self.rows,
            "XQ packed main KV snapshot",
        )?;
        let records = Tensor::cat(&[self.data, self.scales], 1).map_err(tensor_error)?;
        let output = MainKvRows::from_records(&self.device, format, &records)?;
        check_cancel(cancellation)?;
        Ok(output)
    }

    pub fn into_index_rows(self, cancellation: &CancellationToken) -> Result<Fp4IndexRows> {
        check_cancel(cancellation)?;
        if self.format != XqActivationFormat::IndexFp4 {
            return Err(invalid(
                "main/window XQ activation cannot become index KV storage",
            ));
        }
        live_bytes(
            &self.device,
            self.format,
            self.rows,
            "XQ packed index snapshot",
        )?;
        let output = Fp4IndexRows::from_packed(&self.device, &self.data, &self.scales)?;
        check_cancel(cancellation)?;
        Ok(output)
    }
}

fn live_bytes(
    device: &CudaLayerDevice,
    format: XqActivationFormat,
    rows: usize,
    label: &str,
) -> Result<()> {
    let bytes = (rows as u64)
        .checked_mul(format.columns() as u64)
        .and_then(|n| n.checked_mul(128))
        .and_then(|n| n.checked_add(4096))
        .ok_or_else(|| invalid("XQ activation live tensor budget overflowed"))?;
    device.limits().checked_state_bytes(bytes, label)?;
    Ok(())
}

#[cfg(test)]
pub(in crate::layers::xq) mod tests;
