//! Bounded, hash-bound XQ GGUF header parsing. Does not verify tensor payloads.
//! GGUF v3 layout and XQ metadata follow the pinned Xolotl exporter recorded
//! in the parent module; the corresponding MIT notice is retained.

use std::collections::BTreeMap;
use std::ops::Range;

use aster_power::error::Result;
use sha2::{Digest, Sha256};

use super::{invalid, XqTensorType};

pub const XQ_GGUF_BYTES: u64 = 162_954_903_552;
pub const XQ_HEADER_BYTES: usize = 5_865_472;
pub const XQ_HEADER_SHA256: &str =
    "a8f516bb9bc7304eaf02f018fc6055b701e34cc17e939936a90c38f322abf511";
const MAX_HEADER_BYTES: usize = 32 * 1024 * 1024;
const MAX_STRING_BYTES: usize = 1024 * 1024;
const MAX_ARRAY_ELEMENTS: u64 = 2 * 1024 * 1024;

#[derive(Clone, Debug, PartialEq)]
pub enum XqMetadataValue {
    Unsigned(u64),
    Signed(i64),
    Float(f64),
    Bool(bool),
    String(String),
    /// Raw element values inside the immutable, hash-bound header. Large token
    /// arrays are validated without allocating one Rust object per token.
    Array {
        element_type: u32,
        elements: u64,
        range: Range<usize>,
    },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct XqGgufTensor {
    /// GGUF order: contiguous row width first.
    pub shape: Vec<u64>,
    pub kind: XqTensorType,
    pub absolute_offset: u64,
    pub bytes: u64,
}

#[derive(Debug)]
pub struct XqGgufHeader {
    metadata: BTreeMap<String, XqMetadataValue>,
    tensors: BTreeMap<String, XqGgufTensor>,
    data_offset: u64,
    alignment: u64,
}

impl XqGgufHeader {
    /// Accept exactly the independently audited V3 header, including padding.
    /// A caller must separately bind and verify the complete GGUF payload hash
    /// before loading its ranges. This handle alone cannot activate a backend.
    pub fn from_pinned_bytes(header: &[u8]) -> Result<Self> {
        if header.len() != XQ_HEADER_BYTES
            || format!("{:x}", Sha256::digest(header)) != XQ_HEADER_SHA256
        {
            return Err(invalid("XQ header differs from the pinned V3 artifact"));
        }
        let parsed = Self::parse(header, XQ_GGUF_BYTES)?;
        if parsed.alignment != 16384
            || parsed.tensors.len() != 1329
            || parsed.metadata.get("general.architecture")
                != Some(&XqMetadataValue::String("deepseek41".into()))
        {
            return Err(invalid(
                "XQ header does not satisfy the pinned model schema",
            ));
        }
        Ok(parsed)
    }

    pub fn metadata(&self) -> &BTreeMap<String, XqMetadataValue> {
        &self.metadata
    }

    pub fn tensors(&self) -> &BTreeMap<String, XqGgufTensor> {
        &self.tensors
    }

    pub fn data_offset(&self) -> u64 {
        self.data_offset
    }

    pub fn alignment(&self) -> u64 {
        self.alignment
    }

    pub(super) fn parse(header: &[u8], file_bytes: u64) -> Result<Self> {
        if header.len() > MAX_HEADER_BYTES || header.len() as u64 > file_bytes {
            return Err(invalid(
                "XQ GGUF header exceeds its file or parser byte bound",
            ));
        }
        let mut reader = Reader {
            bytes: header,
            position: 0,
        };
        if reader.take(4)? != b"GGUF" || reader.u32()? != 3 {
            return Err(invalid("XQ requires little-endian GGUF version 3"));
        }
        let tensor_count = reader.u64()?;
        let metadata_count = reader.u64()?;
        if tensor_count == 0
            || tensor_count > 100_000
            || metadata_count == 0
            || metadata_count > 4096
        {
            return Err(invalid("XQ GGUF table counts exceed their bounds"));
        }
        let mut metadata = BTreeMap::new();
        for _ in 0..metadata_count {
            let name = reader.string()?.to_owned();
            if name.is_empty() || name.contains('\0') || metadata.contains_key(&name) {
                return Err(invalid("XQ metadata name is empty, invalid or duplicated"));
            }
            let kind = reader.u32()?;
            metadata.insert(name, reader.value(kind)?);
        }
        let alignment = match metadata.get("general.alignment") {
            Some(XqMetadataValue::Unsigned(value)) => *value,
            None => 32,
            _ => return Err(invalid("XQ GGUF alignment must be an unsigned integer")),
        };
        if alignment == 0 || alignment > 16384 || !alignment.is_power_of_two() {
            return Err(invalid("XQ GGUF alignment exceeds the exporter contract"));
        }
        let mut tensors = BTreeMap::new();
        for _ in 0..tensor_count {
            let name = reader.string()?.to_owned();
            let rank = reader.u32()?;
            if name.is_empty()
                || name.len() > 512
                || name.contains('\0')
                || tensors.contains_key(&name)
                || !(1..=4).contains(&rank)
            {
                return Err(invalid("XQ tensor name or rank is invalid or duplicated"));
            }
            let mut shape = Vec::new();
            for _ in 0..rank {
                shape.push(reader.u64()?);
            }
            let kind = XqTensorType::from_gguf_id(reader.u32()?)?;
            let relative_offset = reader.u64()?;
            if !relative_offset.is_multiple_of(alignment) {
                return Err(invalid("XQ tensor offset is not GGUF-aligned"));
            }
            let bytes = kind.tensor_bytes(&shape)?;
            tensors.insert(
                name,
                XqGgufTensor {
                    shape,
                    kind,
                    absolute_offset: relative_offset,
                    bytes,
                },
            );
        }
        let data_offset = align_up(reader.position as u64, alignment)?;
        let padding = usize::try_from(data_offset - reader.position as u64)
            .map_err(|_| invalid("XQ header padding exceeds usize"))?;
        if reader.take(padding)?.iter().any(|byte| *byte != 0) || reader.position != header.len() {
            return Err(invalid(
                "XQ header has nonzero padding, truncation or trailing bytes",
            ));
        }
        let mut ranges = Vec::new();
        for tensor in tensors.values_mut() {
            tensor.absolute_offset = tensor
                .absolute_offset
                .checked_add(data_offset)
                .ok_or_else(|| invalid("XQ tensor absolute offset overflowed"))?;
            let end = tensor
                .absolute_offset
                .checked_add(tensor.bytes)
                .ok_or_else(|| invalid("XQ tensor end overflowed"))?;
            if end > file_bytes {
                return Err(invalid("XQ tensor range exceeds the declared artifact"));
            }
            ranges.push((tensor.absolute_offset, end));
        }
        ranges.sort_unstable();
        let mut previous_end = data_offset;
        for (start, end) in ranges {
            if start < previous_end {
                return Err(invalid("XQ GGUF tensor ranges overlap"));
            }
            previous_end = end;
        }
        Ok(Self {
            metadata,
            tensors,
            data_offset,
            alignment,
        })
    }
}

fn align_up(value: u64, alignment: u64) -> Result<u64> {
    value
        .checked_add(alignment - 1)
        .map(|v| v / alignment * alignment)
        .ok_or_else(|| invalid("XQ GGUF alignment overflowed"))
}

struct Reader<'a> {
    bytes: &'a [u8],
    position: usize,
}

impl<'a> Reader<'a> {
    fn take(&mut self, count: usize) -> Result<&'a [u8]> {
        let end = self
            .position
            .checked_add(count)
            .ok_or_else(|| invalid("XQ GGUF cursor overflowed"))?;
        let value = self
            .bytes
            .get(self.position..end)
            .ok_or_else(|| invalid("XQ GGUF header is truncated"))?;
        self.position = end;
        Ok(value)
    }

    fn u8(&mut self) -> Result<u8> {
        Ok(self.take(1)?[0])
    }
    fn u16(&mut self) -> Result<u16> {
        let b = self.take(2)?;
        Ok(u16::from_le_bytes([b[0], b[1]]))
    }
    fn u32(&mut self) -> Result<u32> {
        let b = self.take(4)?;
        Ok(u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
    }
    fn u64(&mut self) -> Result<u64> {
        let b = self.take(8)?;
        Ok(u64::from_le_bytes([
            b[0], b[1], b[2], b[3], b[4], b[5], b[6], b[7],
        ]))
    }
    fn string(&mut self) -> Result<&'a str> {
        let count = self.u64()?;
        if count > MAX_STRING_BYTES as u64 {
            return Err(invalid("XQ GGUF string exceeds its byte bound"));
        }
        std::str::from_utf8(self.take(count as usize)?)
            .map_err(|_| invalid("XQ GGUF string is not valid UTF-8"))
    }

    fn value(&mut self, kind: u32) -> Result<XqMetadataValue> {
        use XqMetadataValue as V;
        Ok(match kind {
            0 => V::Unsigned(u64::from(self.u8()?)),
            1 => V::Signed(i64::from(self.u8()? as i8)),
            2 => V::Unsigned(u64::from(self.u16()?)),
            3 => V::Signed(i64::from(self.u16()? as i16)),
            4 => V::Unsigned(u64::from(self.u32()?)),
            5 => V::Signed(i64::from(self.u32()? as i32)),
            6 => V::Float(finite(f64::from(f32::from_bits(self.u32()?)))?),
            7 => V::Bool(match self.u8()? {
                0 => false,
                1 => true,
                _ => return Err(invalid("XQ GGUF boolean must be zero or one")),
            }),
            8 => V::String(self.string()?.to_owned()),
            9 => {
                let element_type = self.u32()?;
                let elements = self.u64()?;
                if element_type > 12 || element_type == 9 || elements > MAX_ARRAY_ELEMENTS {
                    return Err(invalid("XQ GGUF array type or length exceeds its bound"));
                }
                let start = self.position;
                for _ in 0..elements {
                    if element_type == 8 {
                        self.string()?;
                    } else {
                        self.value(element_type)?;
                    }
                }
                V::Array {
                    element_type,
                    elements,
                    range: start..self.position,
                }
            }
            10 => V::Unsigned(self.u64()?),
            11 => V::Signed(self.u64()? as i64),
            12 => V::Float(finite(f64::from_bits(self.u64()?))?),
            _ => {
                return Err(invalid(
                    "unsupported XQ GGUF metadata type; supported set: 0 through 12",
                ))
            }
        })
    }
}

fn finite(value: f64) -> Result<f64> {
    if !value.is_finite() {
        return Err(invalid("XQ GGUF metadata is non-finite"));
    }
    Ok(value)
}

#[cfg(test)]
mod tests;
