//! XQ model ranges over Power's verified, format-neutral storage owner.

use aster_power::error::Result;
use aster_power::inference::{RawWeightRangeRead, VerifiedRawWeightFile};
use tokio_util::sync::CancellationToken;

use super::gguf::{XqGgufHeader, XqGgufTensor, XqMetadataValue, XQ_GGUF_BYTES, XQ_HEADER_BYTES};
use super::{invalid, XQ_GGUF_SHA256};

/// The exact XQ collection, including its verified full payload identity.
///
/// Construction does not register a backend, decode to F32 or upload weights.
/// The host must explicitly admit this file's bytes and the read budget in
/// Power before binding it here. There is no model-owned filesystem I/O.
pub struct XqWeights {
    source: VerifiedRawWeightFile,
    header: XqGgufHeader,
}

/// Original packed bytes for complete contiguous GGUF rows.
///
/// GGUF puts the contiguous column dimension first. For stacked experts,
/// the row index is `expert_slot * output_rows + output_row`. The returned
/// descriptor covers the whole tensor; `first_row`/`rows` identify this read.
pub struct XqTensorRead {
    raw: RawWeightRangeRead,
    tensor: XqGgufTensor,
    first_row: u64,
    rows: u64,
}

impl XqWeights {
    pub fn from_verified_file(
        source: VerifiedRawWeightFile,
        cancellation: &CancellationToken,
    ) -> Result<Self> {
        let identity = source.descriptor();
        if identity.bytes != XQ_GGUF_BYTES || identity.sha256 != XQ_GGUF_SHA256 {
            return Err(invalid(
                "XQ payload differs from the pinned complete GGUF artifact",
            ));
        }
        let header_bytes = source.read_range(0, XQ_HEADER_BYTES as u64, cancellation)?;
        let header = XqGgufHeader::from_pinned_bytes(header_bytes.bytes())?;
        Ok(Self { source, header })
    }

    pub fn header(&self) -> &XqGgufHeader {
        &self.header
    }

    /// Pinned model artifact metadata, not product configuration or a loaded
    /// backend. The complete GGUF payload remains owned by this verified file.
    pub fn model_config(&self) -> Result<crate::model::VerifiedCheckpointConfig> {
        let Some(XqMetadataValue::String(config)) = self.header.metadata().get("deepseek41.config")
        else {
            return Err(invalid(
                "XQ header is missing its exact model configuration metadata",
            ));
        };
        crate::model::DeepSeekModel::V41Flash1017Xq.verify_config(config.as_bytes())
    }

    pub fn max_read_bytes(&self) -> u64 {
        self.source.max_read_bytes()
    }

    /// Typed callers specify an exact schema, never a guessed array length.
    /// The byte range comes only from the already validated GGUF parser and
    /// is authenticated again by Power, including mutations after open.
    pub(crate) fn read_metadata_array(
        &self,
        name: &str,
        element_type: u32,
        elements: usize,
        cancellation: &CancellationToken,
    ) -> Result<RawWeightRangeRead> {
        let width = match element_type {
            4 => 4_usize,
            10 => 8,
            _ => return Err(invalid("XQ integer array type requires U32 or U64")),
        };
        let Some(XqMetadataValue::Array {
            element_type: actual_type,
            elements: actual_elements,
            range,
        }) = self.header.metadata().get(name)
        else {
            return Err(invalid("XQ required integer metadata array is absent"));
        };
        let bytes = elements
            .checked_mul(width)
            .ok_or_else(|| invalid("XQ metadata array byte count overflowed"))?;
        if elements == 0
            || *actual_type != element_type
            || *actual_elements != elements as u64
            || range.end.checked_sub(range.start) != Some(bytes)
            || range.end as u64 > self.header.data_offset()
        {
            return Err(invalid(
                "XQ integer metadata array differs from its exact schema",
            ));
        }
        self.source
            .read_range(range.start as u64, bytes as u64, cancellation)
    }

    pub fn tensor(&self, name: &str) -> Result<&XqGgufTensor> {
        self.header
            .tensors()
            .get(name)
            .ok_or_else(|| invalid("XQ tensor is absent from the pinned catalog"))
    }

    /// Materializes only the requested complete rows, retaining packed type
    /// bytes and scales. Budget refusal is explicit; there is no full-tensor
    /// decode or alternate source fallback for an oversized range.
    pub fn read_rows(
        &self,
        name: &str,
        first_row: u64,
        rows: u64,
        cancellation: &CancellationToken,
    ) -> Result<XqTensorRead> {
        let tensor = self.tensor(name)?;
        let (offset, bytes) = row_range(tensor, first_row, rows)?;
        let raw = self.source.read_range(offset, bytes, cancellation)?;
        Ok(XqTensorRead {
            raw,
            tensor: tensor.clone(),
            first_row,
            rows,
        })
    }

    pub fn read_tensor(
        &self,
        name: &str,
        cancellation: &CancellationToken,
    ) -> Result<XqTensorRead> {
        let tensor = self.tensor(name)?;
        let row_bytes = tensor.kind.row_bytes(tensor.shape[0])?;
        self.read_rows(name, 0, tensor.bytes / row_bytes, cancellation)
    }
}

impl XqTensorRead {
    pub fn bytes(&self) -> &[u8] {
        self.raw.bytes()
    }

    pub fn tensor(&self) -> &XqGgufTensor {
        &self.tensor
    }

    pub fn first_row(&self) -> u64 {
        self.first_row
    }

    pub fn rows(&self) -> u64 {
        self.rows
    }

    pub fn absolute_offset(&self) -> u64 {
        self.raw.offset()
    }
}

impl std::fmt::Debug for XqWeights {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("XqWeights")
            .field("tensors", &self.header.tensors().len())
            .field("max_read_bytes", &self.source.max_read_bytes())
            .finish_non_exhaustive()
    }
}

impl std::fmt::Debug for XqTensorRead {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("XqTensorRead")
            .field("kind", &self.tensor.kind)
            .field("bytes", &self.raw.bytes().len())
            .field("first_row", &self.first_row)
            .field("rows", &self.rows)
            .finish_non_exhaustive()
    }
}

fn row_range(tensor: &XqGgufTensor, first: u64, rows: u64) -> Result<(u64, u64)> {
    let columns = *tensor
        .shape
        .first()
        .ok_or_else(|| invalid("XQ row range requires a non-empty tensor shape"))?;
    let row_bytes = tensor.kind.row_bytes(columns)?;
    let end = first
        .checked_add(rows)
        .ok_or_else(|| invalid("XQ row range overflowed"))?;
    if rows == 0 || end > tensor.bytes / row_bytes {
        return Err(invalid(
            "XQ row range is empty or exceeds its verified tensor",
        ));
    }
    let offset = first
        .checked_mul(row_bytes)
        .and_then(|relative| tensor.absolute_offset.checked_add(relative))
        .ok_or_else(|| invalid("XQ row absolute offset overflowed"))?;
    let bytes = rows
        .checked_mul(row_bytes)
        .ok_or_else(|| invalid("XQ row byte count overflowed"))?;
    offset
        .checked_add(bytes)
        .ok_or_else(|| invalid("XQ row absolute end overflowed"))?;
    Ok((offset, bytes))
}

#[cfg(test)]
pub(crate) mod tests;
