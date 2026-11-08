//! Two pinned official Engram shards over Power's held, authenticated files.
//! Canonical blob paths are explicit host inputs; snapshot symlinks are never
//! opened here. No copy of the official trunk or giant table is required.

use std::collections::BTreeMap;

use aster_power::{
    error::Result,
    inference::{RawWeightRangeRead, VerifiedRawWeightFile},
};
use serde::Deserialize;
use tokio_util::sync::CancellationToken;

use super::{invalid, official::OFFICIAL_SHARDS, CheckpointWeights, MatrixLayout};
use crate::engram::EngramLayer;
use crate::layers::{engram::lookup::EngramTable, CudaLayerDevice};

#[derive(Clone, Copy)]
pub enum XqEngramTableSource<'a> {
    Official(&'a CheckpointWeights),
    VerifiedFiles(&'a VerifiedEngramTables),
}

impl<'a> XqEngramTableSource<'a> {
    pub(crate) fn bind(
        self,
        device: &CudaLayerDevice,
        layer: EngramLayer,
    ) -> Result<EngramTable<'a>> {
        match self {
            Self::Official(checkpoint) => {
                let prefix = format!("layers.{}.engram.embed", layer.as_str());
                let matrix = checkpoint.matrix(
                    &format!("{prefix}.weight"),
                    Some(&format!("{prefix}.scale")),
                    MatrixLayout::Fp8Row32,
                    [layer.table_rows() as usize, 256],
                )?;
                EngramTable::new(device.clone(), matrix)
            }
            Self::VerifiedFiles(tables) => {
                EngramTable::from_raw(device.clone(), tables.matrix(layer)?)
            }
        }
    }
}

pub struct VerifiedEngramTables {
    files: [RawEngramFile; 2],
}

struct RawEngramFile {
    layer: EngramLayer,
    source: VerifiedRawWeightFile,
    rows: usize,
    weight_start: u64,
    scale_start: u64,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct TensorRecord {
    dtype: String,
    shape: Vec<usize>,
    data_offsets: [u64; 2],
}

pub(crate) struct RawEngramMatrix<'a>(&'a RawEngramFile);
pub(crate) struct RawEngramRows {
    pub weight: RawWeightRangeRead,
    pub scale: RawWeightRangeRead,
    pub rows: usize,
}

impl VerifiedEngramTables {
    /// Each Power source has already authenticated its complete bytes. Only
    /// official shards 47/48 are accepted, in that explicit order; physical
    /// filenames do not replace their full SHA256/length identities.
    pub fn from_verified_files(
        files: [VerifiedRawWeightFile; 2],
        cancellation: &CancellationToken,
    ) -> Result<Self> {
        for (source, &(.., bytes, sha256)) in files.iter().zip(&OFFICIAL_SHARDS[46..48]) {
            if source.descriptor().bytes != bytes || source.descriptor().sha256 != sha256 {
                return Err(invalid(
                    "Engram raw source differs from the two pinned complete official shards",
                ));
            }
        }
        let [first, second] = files;
        Ok(Self {
            files: [
                RawEngramFile::parse(
                    first,
                    EngramLayer::Layer1,
                    EngramLayer::Layer1.table_rows() as usize,
                    cancellation,
                )?,
                RawEngramFile::parse(
                    second,
                    EngramLayer::Layer14,
                    EngramLayer::Layer14.table_rows() as usize,
                    cancellation,
                )?,
            ],
        })
    }

    pub(crate) fn matrix(&self, layer: EngramLayer) -> Result<RawEngramMatrix<'_>> {
        self.files
            .iter()
            .find(|source| source.layer == layer)
            .map(RawEngramMatrix)
            .ok_or_else(|| invalid("verified Engram collection is missing its explicit layer"))
    }
}

impl RawEngramFile {
    fn parse(
        source: VerifiedRawWeightFile,
        layer: EngramLayer,
        rows: usize,
        cancellation: &CancellationToken,
    ) -> Result<Self> {
        let raw = source.read_range(0, 8, cancellation)?;
        let header_bytes = u64::from_le_bytes(
            raw.bytes()
                .try_into()
                .map_err(|_| invalid("Engram SafeTensors length is truncated"))?,
        );
        if header_bytes == 0
            || header_bytes > 4096
            || rows == 0
            || rows > layer.table_rows() as usize
        {
            return Err(invalid(
                "Engram SafeTensors header/row count exceeds its exact bounded schema",
            ));
        }
        let header = source.read_range(8, header_bytes, cancellation)?;
        let (weight_start, scale_start) =
            validate_catalog(header.bytes(), layer, rows, source.descriptor().bytes)?;
        Ok(Self {
            layer,
            rows,
            weight_start,
            scale_start,
            source,
        })
    }
}

fn validate_catalog(
    header: &[u8],
    layer: EngramLayer,
    rows: usize,
    file_bytes: u64,
) -> Result<(u64, u64)> {
    if header.is_empty() || header.len() > 4096 || rows == 0 || rows > layer.table_rows() as usize {
        return Err(invalid(
            "Engram catalog exceeds its exact metadata/row bounds",
        ));
    }
    let records: BTreeMap<String, TensorRecord> = serde_json::from_slice(header)
        .map_err(|_| invalid("Engram SafeTensors header has an unknown tensor schema"))?;
    let expected = [
        ("embed.weight", "F8_E4M3", vec![rows, 256], 1_u64),
        ("embed.scale", "F8_E8M0", vec![rows, 8], 1),
        ("q_weight", "BF16", vec![4, 5120], 2),
        ("k_weight", "BF16", vec![4, 5120], 2),
        ("wkv.weight", "F8_E4M3", vec![25600, 6144], 1),
        ("wkv.scale", "F8_E8M0", vec![800, 192], 1),
    ];
    if records.len() != expected.len() {
        return Err(invalid(
            "Engram shard must contain exactly its six pinned tensors",
        ));
    }
    let mut offset = 0_u64;
    for (suffix, dtype, shape, width) in expected {
        let name = format!("layers.{}.engram.{suffix}", layer.as_str());
        let tensor = records
            .get(&name)
            .ok_or_else(|| invalid("Engram shard is missing a pinned tensor name"))?;
        let bytes = shape
            .iter()
            .try_fold(width, |total, &extent| total.checked_mul(extent as u64))
            .ok_or_else(|| invalid("Engram tensor byte arithmetic overflowed"))?;
        let end = offset
            .checked_add(bytes)
            .ok_or_else(|| invalid("Engram tensor offset overflowed"))?;
        if tensor.dtype != dtype || tensor.shape != shape || tensor.data_offsets != [offset, end] {
            return Err(invalid("Engram tensor dtype, shape or contiguous payload extent differs from its pinned schema"));
        }
        offset = end;
    }
    let payload_start = 8 + header.len() as u64;
    if payload_start.checked_add(offset) != Some(file_bytes) {
        return Err(invalid(
            "Engram tensor payload does not cover its exact source file",
        ));
    }
    Ok((payload_start, payload_start + rows as u64 * 256))
}

impl RawEngramMatrix<'_> {
    pub(crate) fn rows(&self) -> usize {
        self.0.rows
    }
    pub(crate) fn read_budget(&self) -> u64 {
        self.0.source.max_read_bytes()
    }
    pub(crate) fn read_rows(
        &self,
        first: usize,
        rows: usize,
        cancellation: &CancellationToken,
    ) -> Result<RawEngramRows> {
        if rows == 0 || first.checked_add(rows).is_none_or(|end| end > self.0.rows) {
            return Err(invalid("Engram raw rows exceed the verified table extent"));
        }
        let bytes = (rows as u64)
            .checked_mul(264)
            .ok_or_else(|| invalid("Engram row bytes overflowed"))?;
        if bytes > self.read_budget() {
            return Err(invalid(
                "Engram raw weight and scale rows exceed the joint read budget",
            ));
        }
        let weight = self.0.source.read_range(
            self.0.weight_start + first as u64 * 256,
            rows as u64 * 256,
            cancellation,
        )?;
        let scale = self.0.source.read_range(
            self.0.scale_start + first as u64 * 8,
            rows as u64 * 8,
            cancellation,
        )?;
        Ok(RawEngramRows {
            weight,
            scale,
            rows,
        })
    }
}

#[cfg(test)]
mod tests;
