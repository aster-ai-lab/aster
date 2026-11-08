//! Exact XQ V3 block layouts, independent of native model activation.
//!
//! Layouts follow Xolotl b3452666e07a6374d06bb8c2185dffa271e8f65f,
//! tools/v41/v41_formats.py and include/strata/kernels/cpu/ptq1_0.hpp.
//! Copyright (c) 2026 Niko1221 and the Strata contributors; MIT notice:
//! licenses/Xolotl-MIT.txt. These bounded CPU codecs support format validation
//! and row references. They neither register an XQ backend nor provide a CPU
//! fallback for native CUDA inference. GGUF/file integrity and CUDA dispatch
//! are separate obligations.

use aster_power::error::Result;

use super::invalid;

pub mod experts;
pub mod gguf;
pub mod load;
pub mod parameters;
pub mod storage;

pub const XQ_REPOSITORY: &str = "IndrasMirror/DeepSeek-V4.1-Flash-1017-XQ-Uncensored";
pub const XQ_REVISION: &str = "9d3c01d86709261e51d566b02e1dda75f40d009a";
pub const XQ_GGUF_SHA256: &str = "73df1934302f867601dd27f7fca59109d9b11674f50cc60526ec2a1cd4f3d389";

/// GGUF type IDs from this pinned exporter. Unknown IDs never mean F32.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum XqTensorType {
    F32,
    F16,
    Q8_0,
    Q3K,
    I8,
    I32,
    Q2_0,
    Ptq1_0,
}

impl XqTensorType {
    pub fn parse(value: &str) -> Result<Self> {
        match value {
            "F32" => Ok(Self::F32),
            "F16" => Ok(Self::F16),
            "Q8_0" => Ok(Self::Q8_0),
            "Q3_K" => Ok(Self::Q3K),
            "I8" => Ok(Self::I8),
            "I32" => Ok(Self::I32),
            "Q2_0" => Ok(Self::Q2_0),
            "PTQ1_0" => Ok(Self::Ptq1_0),
            _ => Err(invalid(
                "unsupported XQ type; supported set: F32, F16, Q8_0, Q3_K, I8, I32, Q2_0, PTQ1_0",
            )),
        }
    }

    pub fn from_gguf_id(value: u32) -> Result<Self> {
        match value {
            0 => Ok(Self::F32),
            1 => Ok(Self::F16),
            8 => Ok(Self::Q8_0),
            11 => Ok(Self::Q3K),
            24 => Ok(Self::I8),
            26 => Ok(Self::I32),
            42 => Ok(Self::Q2_0),
            143 => Ok(Self::Ptq1_0),
            _ => Err(invalid(
                "unsupported XQ GGUF type ID; supported set: 0, 1, 8, 11, 24, 26, 42, 143",
            )),
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::F32 => "F32",
            Self::F16 => "F16",
            Self::Q8_0 => "Q8_0",
            Self::Q3K => "Q3_K",
            Self::I8 => "I8",
            Self::I32 => "I32",
            Self::Q2_0 => "Q2_0",
            Self::Ptq1_0 => "PTQ1_0",
        }
    }

    pub fn gguf_id(self) -> u32 {
        match self {
            Self::F32 => 0,
            Self::F16 => 1,
            Self::Q8_0 => 8,
            Self::Q3K => 11,
            Self::I8 => 24,
            Self::I32 => 26,
            Self::Q2_0 => 42,
            Self::Ptq1_0 => 143,
        }
    }

    /// (logical elements, stored bytes), including in-block FP16 scales.
    pub fn block_layout(self) -> (u64, u64) {
        match self {
            Self::F32 | Self::I32 => (1, 4),
            Self::F16 => (1, 2),
            Self::I8 => (1, 1),
            Self::Q8_0 => (32, 34),
            Self::Q3K => (256, 110),
            Self::Q2_0 => (64, 18),
            Self::Ptq1_0 => (128, 28),
        }
    }

    pub fn row_bytes(self, columns: u64) -> Result<u64> {
        let (elements, bytes) = self.block_layout();
        if columns == 0 || !columns.is_multiple_of(elements) {
            return Err(invalid(
                "XQ row requires positive complete quantization blocks",
            ));
        }
        (columns / elements)
            .checked_mul(bytes)
            .ok_or_else(|| invalid("XQ row byte count overflowed"))
    }

    /// GGUF dimension order: ne0 is the contiguous row width.
    pub fn tensor_bytes(self, shape: &[u64]) -> Result<u64> {
        if shape.is_empty() || shape.len() > 4 || shape.contains(&0) {
            return Err(invalid(
                "XQ tensor requires one through four positive dimensions",
            ));
        }
        shape[1..]
            .iter()
            .try_fold(self.row_bytes(shape[0])?, |n, d| {
                n.checked_mul(*d)
                    .ok_or_else(|| invalid("XQ tensor byte count overflowed"))
            })
    }

    /// Validate floating payloads without allocating decoded rows. Native
    /// upload uses this to reject non-finite scalar values or block scales
    /// while retaining every original packed byte.
    pub fn validate_floating_payload(self, raw: &[u8], columns: u64, rows: u64) -> Result<()> {
        if matches!(self, Self::I8 | Self::I32) {
            return Err(invalid(
                "XQ integer metadata cannot enter a floating projection",
            ));
        }
        let bytes = self
            .row_bytes(columns)?
            .checked_mul(rows)
            .ok_or_else(|| invalid("XQ floating payload size overflowed"))?;
        if rows == 0 || bytes != raw.len() as u64 {
            return Err(invalid(
                "XQ floating payload requires exact non-empty complete rows",
            ));
        }
        let block_bytes = self.block_layout().1 as usize;
        for block in raw.chunks_exact(block_bytes) {
            match self {
                Self::F32 => {
                    finite(f32::from_le_bytes([block[0], block[1], block[2], block[3]]))?;
                }
                Self::F16 | Self::Q8_0 | Self::Q2_0 => {
                    half(block, 0)?;
                }
                Self::Q3K => {
                    half(block, 108)?;
                }
                Self::Ptq1_0 => {
                    half(block, 26)?;
                }
                Self::I8 | Self::I32 => {
                    return Err(invalid(
                        "XQ integer payload cannot enter floating validation",
                    ))
                }
            }
        }
        Ok(())
    }

    /// Decode exactly one bounded row. Caller owns the explicit output budget.
    /// Integer metadata must use an integer reader, preserving all 32 bits.
    pub fn decode_f32_row(
        self,
        raw: &[u8],
        columns: usize,
        max_output_bytes: u64,
    ) -> Result<Vec<f32>> {
        if matches!(self, Self::I8 | Self::I32) {
            return Err(invalid(
                "XQ integer metadata requires a typed integer reader",
            ));
        }
        validate_row(self, raw, columns, max_output_bytes)?;
        let mut output = Vec::new();
        output
            .try_reserve_exact(columns)
            .map_err(|_| invalid("XQ decoded row allocation exceeds available memory"))?;
        let (_, block_bytes) = self.block_layout();
        for block in raw.chunks_exact(block_bytes as usize) {
            match self {
                Self::F32 => output.push(finite(f32::from_le_bytes([
                    block[0], block[1], block[2], block[3],
                ]))?),
                Self::F16 => output.push(half(block, 0)?),
                Self::Q8_0 => {
                    let scale = half(block, 0)?;
                    output.extend(block[2..].iter().map(|q| (*q as i8) as f32 * scale));
                }
                Self::Q2_0 => {
                    let scale = half(block, 0)?;
                    for packed in &block[2..] {
                        for shift in [0, 2, 4, 6] {
                            output.push((((packed >> shift) & 3) as i32 - 1) as f32 * scale);
                        }
                    }
                }
                Self::Ptq1_0 => {
                    let scale = half(block, 26)?;
                    for (start, count, trits) in [(0, 16, 5), (16, 8, 5), (24, 2, 4)] {
                        for power in [1_u16, 3, 9, 27, 81].iter().take(trits) {
                            for byte in &block[start..start + count] {
                                let digit = (((u16::from(*byte) * power) & 255) * 3) >> 8;
                                output.push((i32::from(digit) - 1) as f32 * scale);
                            }
                        }
                    }
                }
                Self::Q3K => {
                    let scale = half(block, 108)?;
                    let mut subscales = [0_i32; 16];
                    for j in 0..4 {
                        let a = block[96 + j];
                        let c = block[100 + j];
                        let high = block[104 + j];
                        subscales[j] = i32::from((a & 15) | ((high & 3) << 4)) - 32;
                        subscales[4 + j] = i32::from((c & 15) | (((high >> 2) & 3) << 4)) - 32;
                        subscales[8 + j] = i32::from((a >> 4) | (((high >> 4) & 3) << 4)) - 32;
                        subscales[12 + j] = i32::from((c >> 4) | (((high >> 6) & 3) << 4)) - 32;
                    }
                    for i in 0..256 {
                        let h = i / 128;
                        let j = (i % 128) / 32;
                        let lane = i % 32;
                        let low = (block[32 + h * 32 + lane] >> (2 * j)) & 3;
                        let high = (block[lane] >> (4 * h + j)) & 1;
                        let code = i32::from(low) - if high == 0 { 4 } else { 0 };
                        output.push((code as f32 * scale) * subscales[i / 16] as f32);
                    }
                }
                Self::I8 | Self::I32 => {
                    return Err(invalid(
                        "XQ integer metadata cannot enter floating decoding",
                    ));
                }
            }
        }
        Ok(output)
    }

    pub fn decode_i32_row(
        self,
        raw: &[u8],
        columns: usize,
        max_output_bytes: u64,
    ) -> Result<Vec<i32>> {
        if self != Self::I32 {
            return Err(invalid(
                "XQ I32 reader requires the explicit I32 metadata type",
            ));
        }
        validate_row(self, raw, columns, max_output_bytes)?;
        let mut output = Vec::new();
        output
            .try_reserve_exact(columns)
            .map_err(|_| invalid("XQ I32 row allocation exceeds available memory"))?;
        for b in raw.chunks_exact(4) {
            output.push(i32::from_le_bytes([b[0], b[1], b[2], b[3]]));
        }
        Ok(output)
    }
}

fn validate_row(format: XqTensorType, raw: &[u8], columns: usize, budget: u64) -> Result<()> {
    let columns = u64::try_from(columns).map_err(|_| invalid("XQ row width exceeds u64"))?;
    let expected = format.row_bytes(columns)?;
    let output_bytes = columns
        .checked_mul(4)
        .ok_or_else(|| invalid("XQ decoded row byte count overflowed"))?;
    if expected != raw.len() as u64 || output_bytes > budget {
        return Err(invalid(
            "XQ row is truncated, has trailing bytes or exceeds its output budget",
        ));
    }
    Ok(())
}

fn half(bytes: &[u8], offset: usize) -> Result<f32> {
    let raw = u16::from_le_bytes([bytes[offset], bytes[offset + 1]]);
    let sign = u32::from(raw & 0x8000) << 16;
    let exponent = (raw >> 10) & 31;
    let fraction = u32::from(raw & 1023);
    match exponent {
        0 => {
            let value = fraction as f32 * (1.0 / 16_777_216.0);
            Ok(f32::from_bits(value.to_bits() | sign))
        }
        31 => Err(invalid("XQ FP16 value or block scale is non-finite")),
        _ => Ok(f32::from_bits(
            sign | ((u32::from(exponent) + 112) << 23) | (fraction << 13),
        )),
    }
}

fn finite(value: f32) -> Result<f32> {
    if !value.is_finite() {
        return Err(invalid("XQ F32 value is non-finite"));
    }
    Ok(value)
}

#[cfg(test)]
mod tests;
