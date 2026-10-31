//! Official checkpoint binding and bounded reads that preserve quantized bytes.
//!
//! Power verifies and owns the storage. This module binds its verified file
//! hashes to the pinned upstream checkpoint, then supplies explicit model
//! layouts. An I8 container is never inferred to mean an INT8 matrix.

use std::collections::{BTreeMap, BTreeSet};

use aster_power::error::{PowerError, Result};
use aster_power::inference::{
    TensorRangeRead, TensorStorageDescriptor, WeightFileDescriptor, WeightStore,
};
use serde::Deserialize;
use sha2::{Digest, Sha256};
use tokio_util::sync::CancellationToken;

use crate::model::{VerifiedCheckpointConfig, OFFICIAL_INDEX_SHA256};

pub mod engram;
mod official;
pub mod text;
pub mod xq;

const MAX_INDEX_BYTES: usize = 16 * 1024 * 1024;
const MAX_INDEX_TENSORS: usize = 100_000;

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct CheckpointIndex {
    metadata: IndexMetadata,
    weight_map: BTreeMap<String, String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct IndexMetadata {
    total_size: u64,
}

/// A hash-bound official collection. Creating it does not upload weights or
/// register a backend. The input store must contain regular files under Power's
/// trusted roots; HF snapshot symlinks must be materialized by the deployment
/// owner without relaxing Power's path rules.
pub struct CheckpointWeights {
    store: WeightStore,
    read_budget_bytes: u64,
}

impl CheckpointWeights {
    pub fn from_store(
        store: WeightStore,
        _config: &VerifiedCheckpointConfig,
        index_bytes: &[u8],
        read_budget_bytes: u64,
    ) -> Result<Self> {
        let index = parse_index(index_bytes, OFFICIAL_INDEX_SHA256)?;
        let files = official::OFFICIAL_SHARDS
            .iter()
            .map(|(name, bytes, sha256)| WeightFileDescriptor {
                relative_path: (*name).into(),
                bytes: *bytes,
                sha256: (*sha256).into(),
            })
            .collect::<Vec<_>>();
        Self::bind(store, index, &files, read_budget_bytes)
    }

    fn bind(
        store: WeightStore,
        index: CheckpointIndex,
        expected_files: &[WeightFileDescriptor],
        read_budget_bytes: u64,
    ) -> Result<Self> {
        if read_budget_bytes == 0 {
            return Err(invalid("weight read budget must be positive"));
        }
        validate_files(store.files(), expected_files)?;
        if store.inventory().len() != index.weight_map.len() {
            return Err(invalid(
                "checkpoint tensor inventory does not match its index",
            ));
        }
        let mut total_bytes = 0_u64;
        for tensor in store.inventory() {
            let shard = index
                .weight_map
                .get(&tensor.name)
                .ok_or_else(|| invalid("checkpoint contains a tensor absent from its index"))?;
            let storage = store.storage_descriptor(&tensor.name).ok_or_else(|| {
                invalid("checkpoint tensor is missing its verified storage descriptor")
            })?;
            if store
                .files()
                .get(storage.file_index)
                .map(|file| file.relative_path.as_str())
                != Some(shard.as_str())
            {
                return Err(invalid(
                    "checkpoint index assigns a tensor to a different shard",
                ));
            }
            total_bytes = total_bytes
                .checked_add(storage.bytes)
                .ok_or_else(|| invalid("checkpoint tensor byte sum overflowed"))?;
        }
        if total_bytes != index.metadata.total_size {
            return Err(invalid(
                "checkpoint tensor byte total differs from its index",
            ));
        }
        Ok(Self {
            store,
            read_budget_bytes,
        })
    }

    pub fn collection_sha256(&self) -> &str {
        self.store.sha256()
    }

    pub fn storage(&self, name: &str) -> Result<TensorStorageDescriptor> {
        self.store
            .storage_descriptor(name)
            .ok_or_else(|| invalid("checkpoint tensor is missing"))
    }

    /// A bounded tensor-relative range, including non-matrix state/embedding
    /// tables. It retains Power cancellation, integrity and zeroizing ownership.
    pub fn read_range(
        &self,
        name: &str,
        offset: u64,
        bytes: u64,
        cancellation: &CancellationToken,
    ) -> Result<TensorRangeRead> {
        self.check_read_budget(bytes)?;
        self.store
            .read_tensor_range_with_cancellation(name, offset, bytes, cancellation)
    }

    /// Layout and logical shape come from the model architecture, never a name
    /// heuristic. Scale names are explicit checkpoint entities supplied by it.
    pub fn matrix(
        &self,
        weight: &str,
        scale: Option<&str>,
        layout: MatrixLayout,
        logical_shape: [usize; 2],
    ) -> Result<MatrixWeights<'_>> {
        let storage = self.storage(weight)?;
        let scale_storage = scale.map(|name| self.storage(name)).transpose()?;
        let descriptor = MatrixDescriptor::validate(layout, logical_shape, storage, scale_storage)?;
        Ok(MatrixWeights {
            checkpoint: self,
            weight: weight.into(),
            scale: scale.map(str::to_owned),
            descriptor,
        })
    }

    fn check_read_budget(&self, bytes: u64) -> Result<()> {
        if bytes == 0 || bytes > self.read_budget_bytes {
            return Err(invalid("weight read exceeds its nonzero raw-byte budget"));
        }
        Ok(())
    }
}

/// Checkpoint row-major layouts. Kernel-specific repacking is a separate,
/// explicit operation; these descriptors always describe upstream bytes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MatrixLayout {
    Bf16,
    F32,
    Fp8Block32,
    Fp8Row32,
    Mxfp4Block32,
}

impl MatrixLayout {
    pub fn parse(value: &str) -> Result<Self> {
        match value {
            "bf16" => Ok(Self::Bf16), "f32" => Ok(Self::F32),
            "fp8_e4m3_block32_ue8m0" => Ok(Self::Fp8Block32),
            "fp8_e4m3_row32_ue8m0" => Ok(Self::Fp8Row32),
            "mxfp4_e2m1_block32_ue8m0" => Ok(Self::Mxfp4Block32),
            _ => Err(invalid("unsupported matrix layout; expected bf16, f32, fp8_e4m3_block32_ue8m0, fp8_e4m3_row32_ue8m0 or mxfp4_e2m1_block32_ue8m0")),
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Bf16 => "bf16",
            Self::F32 => "f32",
            Self::Fp8Block32 => "fp8_e4m3_block32_ue8m0",
            Self::Fp8Row32 => "fp8_e4m3_row32_ue8m0",
            Self::Mxfp4Block32 => "mxfp4_e2m1_block32_ue8m0",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MatrixDescriptor {
    layout: MatrixLayout,
    logical_shape: [usize; 2],
    weight: TensorStorageDescriptor,
    scale: Option<TensorStorageDescriptor>,
}

impl MatrixDescriptor {
    fn validate(
        layout: MatrixLayout,
        logical_shape: [usize; 2],
        weight: TensorStorageDescriptor,
        scale: Option<TensorStorageDescriptor>,
    ) -> Result<Self> {
        let [rows, columns] = logical_shape;
        let elements = checked_product(rows, columns)?;
        let (dtype, physical_shape, weight_bytes, scale_shape) = match layout {
            MatrixLayout::Bf16 => ("bf16", logical_shape, elements.checked_mul(2), None),
            MatrixLayout::F32 => ("f32", logical_shape, elements.checked_mul(4), None),
            MatrixLayout::Fp8Block32 => (
                "f8_e4m3",
                logical_shape,
                Some(elements),
                Some([rows.div_ceil(32), columns.div_ceil(32)]),
            ),
            MatrixLayout::Mxfp4Block32 => {
                if !columns.is_multiple_of(32) {
                    return Err(invalid(
                        "MXFP4 columns must contain complete groups of 32 logical values",
                    ));
                }
                (
                    "i8",
                    [rows, columns / 2],
                    Some(elements / 2),
                    Some([rows, columns / 32]),
                )
            }
            MatrixLayout::Fp8Row32 => {
                if !columns.is_multiple_of(32) {
                    return Err(invalid(
                        "per-row FP8 columns must contain complete groups of 32 values",
                    ));
                }
                (
                    "f8_e4m3",
                    logical_shape,
                    Some(elements),
                    Some([rows, columns / 32]),
                )
            }
        };
        let weight_bytes = weight_bytes.ok_or_else(|| invalid("matrix byte count overflowed"))?;
        validate_storage(&weight, dtype, physical_shape, weight_bytes)?;
        match (scale_shape, scale.as_ref()) {
            (Some(shape), Some(storage)) => validate_storage(
                storage,
                "f8_e8m0",
                shape,
                checked_product(shape[0], shape[1])?,
            )?,
            (None, None) => (),
            _ => {
                return Err(invalid(
                    "matrix scale is missing or forbidden for its declared layout",
                ))
            }
        }
        Ok(Self {
            layout,
            logical_shape,
            weight,
            scale,
        })
    }

    pub fn layout(&self) -> MatrixLayout {
        self.layout
    }
    pub fn logical_shape(&self) -> [usize; 2] {
        self.logical_shape
    }
    pub fn weight(&self) -> &TensorStorageDescriptor {
        &self.weight
    }
    pub fn scale(&self) -> Option<&TensorStorageDescriptor> {
        self.scale.as_ref()
    }
}

pub struct MatrixWeights<'a> {
    checkpoint: &'a CheckpointWeights,
    weight: String,
    scale: Option<String>,
    descriptor: MatrixDescriptor,
}

impl MatrixWeights<'_> {
    pub(crate) fn raw_read_budget_bytes(&self) -> u64 {
        self.checkpoint.read_budget_bytes
    }

    pub fn descriptor(&self) -> &MatrixDescriptor {
        &self.descriptor
    }

    /// Row chunks preserve complete quantization blocks. For tiled FP8, only
    /// the final chunk may end inside a tile; start rows always align to 32.
    pub fn read_rows(
        &self,
        first_row: usize,
        rows: usize,
        cancellation: &CancellationToken,
    ) -> Result<MatrixRows> {
        let descriptor = &self.descriptor;
        let end = first_row
            .checked_add(rows)
            .ok_or_else(|| invalid("matrix row range overflowed"))?;
        let total_rows = descriptor.logical_shape[0];
        if rows == 0 || end > total_rows {
            return Err(invalid("matrix row range exceeds its logical shape"));
        }
        if descriptor.layout == MatrixLayout::Fp8Block32
            && (!first_row.is_multiple_of(32) || (!end.is_multiple_of(32) && end != total_rows))
        {
            return Err(invalid("FP8 row chunks must preserve 32-row scale blocks"));
        }
        let row_bytes = descriptor.weight.bytes / total_rows as u64;
        let offset = (first_row as u64)
            .checked_mul(row_bytes)
            .ok_or_else(|| invalid("matrix row offset overflowed"))?;
        let weight_bytes = (rows as u64)
            .checked_mul(row_bytes)
            .ok_or_else(|| invalid("matrix range bytes overflowed"))?;
        let scale_range = descriptor.scale.as_ref().map(|scale| {
            let (first, count) = if descriptor.layout == MatrixLayout::Fp8Block32 {
                (first_row / 32, end.div_ceil(32) - first_row / 32)
            } else {
                (first_row, rows)
            };
            let width = scale.shape[1] as u64;
            ((first as u64) * width, (count as u64) * width)
        });
        let combined = weight_bytes
            .checked_add(scale_range.map_or(0, |(_, bytes)| bytes))
            .ok_or_else(|| invalid("matrix weight and scale bytes overflowed"))?;
        self.checkpoint.check_read_budget(combined)?;
        let weight =
            self.checkpoint
                .read_range(&self.weight, offset, weight_bytes, cancellation)?;
        let scale = match (&self.scale, scale_range) {
            (Some(name), Some((offset, bytes))) => {
                Some(
                    self.checkpoint
                        .read_range(name, offset, bytes, cancellation)?,
                )
            }
            (None, None) => None,
            _ => return Err(invalid("matrix scale binding is incomplete")),
        };
        if scale
            .as_ref()
            .is_some_and(|scale| scale.bytes().contains(&255))
        {
            return Err(invalid("UE8M0 matrix scale contains a reserved NaN value"));
        }
        Ok(MatrixRows {
            first_row,
            rows,
            weight,
            scale,
        })
    }
}

/// Raw owned tensor ranges, ready for a separately validated CUDA upload.
/// No dequantization, transpose, scale rebasing or implicit precision change.
pub struct MatrixRows {
    pub first_row: usize,
    pub rows: usize,
    pub weight: TensorRangeRead,
    pub scale: Option<TensorRangeRead>,
}

fn validate_storage(
    storage: &TensorStorageDescriptor,
    dtype: &str,
    shape: [usize; 2],
    bytes: u64,
) -> Result<()> {
    if storage.dtype != dtype
        || storage.shape != shape
        || storage.bytes != bytes
        || storage.absolute_offset.checked_add(bytes).is_none()
    {
        return Err(invalid("tensor dtype, physical shape, byte range or quantization block layout differs from the model declaration"));
    }
    Ok(())
}

fn checked_product(rows: usize, columns: usize) -> Result<u64> {
    if rows == 0 || columns == 0 {
        return Err(invalid("matrix dimensions must be positive"));
    }
    let elements = rows
        .checked_mul(columns)
        .ok_or_else(|| invalid("matrix dimensions overflowed"))?;
    u64::try_from(elements).map_err(|_| invalid("matrix element count exceeds u64"))
}

fn parse_index(bytes: &[u8], expected_sha256: &str) -> Result<CheckpointIndex> {
    if bytes.is_empty() || bytes.len() > MAX_INDEX_BYTES {
        return Err(invalid("checkpoint index exceeds its byte bound"));
    }
    if format!("{:x}", Sha256::digest(bytes)) != expected_sha256 {
        return Err(invalid(
            "checkpoint index hash differs from the pinned revision",
        ));
    }
    let index: CheckpointIndex = serde_json::from_slice(bytes)
        .map_err(|_| invalid("checkpoint index has an unsupported artifact schema"))?;
    if index.weight_map.is_empty()
        || index.weight_map.len() > MAX_INDEX_TENSORS
        || index.metadata.total_size == 0
    {
        return Err(invalid(
            "checkpoint index tensor count or byte total is outside its bound",
        ));
    }
    Ok(index)
}

fn validate_files(
    actual: &[WeightFileDescriptor],
    expected: &[WeightFileDescriptor],
) -> Result<()> {
    let expected_names = expected
        .iter()
        .map(|file| file.relative_path.as_str())
        .collect::<BTreeSet<_>>();
    let actual_names = actual
        .iter()
        .map(|file| file.relative_path.as_str())
        .collect::<BTreeSet<_>>();
    if expected.is_empty()
        || expected_names.len() != expected.len()
        || actual.len() != expected.len()
        || actual_names.len() != actual.len()
    {
        return Err(invalid(
            "checkpoint shard set differs from the pinned manifest",
        ));
    }
    for file in actual {
        if !expected.contains(file) {
            return Err(invalid(
                "checkpoint shard name, size or SHA-256 differs from the pinned manifest",
            ));
        }
    }
    Ok(())
}

fn invalid(message: &str) -> PowerError {
    PowerError::InvalidFormat(message.into())
}

#[cfg(test)]
pub(crate) mod tests;
