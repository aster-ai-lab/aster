//! Sparse FP8 table lookup through Power's integrity-bound range reader.
//! Table values/SF stay packed; no BF16 rounding or per-token requantization.
//! Algorithm adapted from DeepInfer (Apache-2.0), revision
//! 9bd9cf243fc53ef9afbf8f04d391d989117d6ab9, v4_modules/engram.py.

use std::collections::BTreeSet;

use aster_power::error::{PowerError, Result};
use candle_core::Tensor;
use tokio_util::sync::CancellationToken;

use crate::{
    engram::{EngramLayer, NgramRow, TABLES_PER_LAYER},
    layers::{invalid, linear::activation::Fp8Activations, tensor_error, CudaLayerDevice},
    weights::{
        engram::{RawEngramMatrix, RawEngramRows},
        MatrixLayout, MatrixRows, MatrixWeights,
    },
};

pub const EMBEDDING_WIDTH: usize = 256;
pub const PROJECTED_INPUT_WIDTH: usize = TABLES_PER_LAYER * EMBEDDING_WIDTH;
const SCALE_WIDTH: usize = EMBEDDING_WIDTH / 32;
const RAW_ROW_BYTES: usize = EMBEDDING_WIDTH + SCALE_WIDTH;

/// Owns the verified table binding, not a full GPU copy of the huge table.
/// HBM residency, caching and distributed gathers are separate T011/T012 work.
pub struct EngramTable<'a> {
    device: CudaLayerDevice,
    weights: EngramWeightSource<'a>,
    rows: usize,
    max_rows_per_read: usize,
}

enum EngramWeightSource<'a> {
    Checkpoint(MatrixWeights<'a>),
    Raw(RawEngramMatrix<'a>),
}
enum EngramRead {
    Checkpoint(MatrixRows),
    Raw(RawEngramRows),
}
impl EngramRead {
    fn values(&self) -> &[u8] {
        match self {
            Self::Checkpoint(rows) => rows.weight.bytes(),
            Self::Raw(rows) => rows.weight.bytes(),
        }
    }
    fn scales(&self) -> Result<&[u8]> {
        match self {
            Self::Checkpoint(rows) => rows
                .scale
                .as_ref()
                .map(|scale| scale.bytes())
                .ok_or_else(|| invalid("Engram table scale binding is absent")),
            Self::Raw(rows) => Ok(rows.scale.bytes()),
        }
    }
    fn rows(&self) -> usize {
        match self {
            Self::Checkpoint(rows) => rows.rows,
            Self::Raw(rows) => rows.rows,
        }
    }
}
impl EngramWeightSource<'_> {
    fn read_rows(
        &self,
        first: usize,
        rows: usize,
        cancellation: &CancellationToken,
    ) -> Result<EngramRead> {
        match self {
            Self::Checkpoint(matrix) => matrix
                .read_rows(first, rows, cancellation)
                .map(EngramRead::Checkpoint),
            Self::Raw(matrix) => matrix
                .read_rows(first, rows, cancellation)
                .map(EngramRead::Raw),
        }
    }
}

impl<'a> EngramTable<'a> {
    pub fn new(device: CudaLayerDevice, weights: MatrixWeights<'a>) -> Result<Self> {
        let [rows, width] = weights.descriptor().logical_shape();
        if weights.descriptor().layout() != MatrixLayout::Fp8Row32
            || width != EMBEDDING_WIDTH
            || rows == 0
            || rows > EngramLayer::Layer14.table_rows() as usize
        {
            return Err(invalid("Engram table requires explicit FP8 row-32 [rows,256] checkpoint storage within the model's row bound"));
        }
        let max_rows_per_read = usize::try_from(
            (weights.raw_read_budget_bytes() / RAW_ROW_BYTES as u64).min(rows as u64),
        )
        .map_err(|_| invalid("Engram raw read row count exceeds usize"))?;
        if max_rows_per_read == 0 {
            return Err(invalid(
                "Engram table raw read budget cannot fit one value/SF row",
            ));
        }
        Ok(Self {
            device,
            weights: EngramWeightSource::Checkpoint(weights),
            rows,
            max_rows_per_read,
        })
    }

    pub(crate) fn from_raw(device: CudaLayerDevice, weights: RawEngramMatrix<'a>) -> Result<Self> {
        let rows = weights.rows();
        let max_rows_per_read =
            usize::try_from((weights.read_budget() / RAW_ROW_BYTES as u64).min(rows as u64))
                .map_err(|_| invalid("Engram raw read row count exceeds usize"))?;
        if rows == 0 || max_rows_per_read == 0 {
            return Err(invalid("Engram raw table requires a nonempty bounded read"));
        }
        Ok(Self {
            device,
            rows,
            max_rows_per_read,
            weights: EngramWeightSource::Raw(weights),
        })
    }

    pub fn table_rows(&self) -> usize {
        self.rows
    }

    /// Bind the exact layer's hash parameters and table extent before fetching.
    /// Reading does not commit n-gram history or any other request state.
    pub fn fetch_ngrams(
        &self,
        layer: EngramLayer,
        ngrams: &[NgramRow],
        cancellation: &CancellationToken,
    ) -> Result<EngramEmbeddings> {
        check_cancel(cancellation)?;
        self.live_tensor_bytes(ngrams.len())?;
        if self.rows != layer.table_rows() as usize {
            return Err(invalid(
                "Engram table extent differs from the explicit hash layer",
            ));
        }
        let hashes = ngrams
            .iter()
            .map(|row| layer.hash(row))
            .collect::<Result<Vec<_>>>()?;
        self.fetch(&hashes, cancellation)
    }

    /// Explicit hash IDs retain head order and duplicate rows. Only an entire
    /// [-1;24] row is an image; no negative ID reaches storage or device gather.
    pub fn fetch(
        &self,
        hashes: &[[i32; TABLES_PER_LAYER]],
        cancellation: &CancellationToken,
    ) -> Result<EngramEmbeddings> {
        check_cancel(cancellation)?;
        let tokens = hashes.len();
        let live_bytes = self.live_tensor_bytes(tokens)?;
        let mut unique = BTreeSet::new();
        let mut image_mask = Vec::with_capacity(tokens);
        for row in hashes {
            check_cancel(cancellation)?;
            let image = row.iter().all(|id| *id == -1);
            image_mask.push(u8::from(image));
            if !image {
                for &id in row {
                    if id < 0 || id as usize >= self.rows {
                        return Err(invalid(
                            "Engram hash is negative/mixed-image or exceeds its bound table",
                        ));
                    }
                    unique.insert(id as u32);
                }
            }
        }
        let unique: Vec<_> = unique.into_iter().collect();
        let mut data_chunks = Vec::new();
        let mut scale_chunks = Vec::new();
        let mut start = 0;
        while start < unique.len() {
            check_cancel(cancellation)?;
            let mut end = start + 1;
            while end < unique.len()
                && end - start < self.max_rows_per_read
                && unique[end] == unique[end - 1] + 1
            {
                end += 1;
            }
            let rows = self
                .weights
                .read_rows(unique[start] as usize, end - start, cancellation)?;
            // Power owns and zeroizes these host ranges. Upload directly from
            // their borrowed slices; never copy payloads into ordinary Vecs.
            data_chunks.push(
                Tensor::from_slice(
                    rows.values(),
                    (rows.rows(), EMBEDDING_WIDTH),
                    self.device.tensor_device(),
                )
                .map_err(tensor_error)?,
            );
            scale_chunks.push(
                Tensor::from_slice(
                    rows.scales()?,
                    (rows.rows(), SCALE_WIDTH),
                    self.device.tensor_device(),
                )
                .map_err(tensor_error)?,
            );
            start = end;
        }
        check_cancel(cancellation)?;
        // A real packed zero row supplies masked image entries. SF=127 means
        // scale one; -1 is never reinterpreted as an unsigned storage offset.
        data_chunks.push(
            Tensor::zeros(
                (1, EMBEDDING_WIDTH),
                candle_core::DType::U8,
                self.device.tensor_device(),
            )
            .map_err(tensor_error)?,
        );
        scale_chunks.push(
            Tensor::full(127_u8, (1, SCALE_WIDTH), self.device.tensor_device())
                .map_err(tensor_error)?,
        );
        let data = Tensor::cat(&data_chunks, 0).map_err(tensor_error)?;
        let scales = Tensor::cat(&scale_chunks, 0).map_err(tensor_error)?;
        let mut row_indices = Vec::with_capacity(tokens * TABLES_PER_LAYER);
        for (row, image) in hashes.iter().zip(&image_mask) {
            check_cancel(cancellation)?;
            for id in row {
                let index = if *image == 1 {
                    unique.len()
                } else {
                    unique
                        .binary_search(&(*id as u32))
                        .map_err(|_| invalid("Engram sparse gather plan lost an existing row"))?
                };
                row_indices.push(
                    u32::try_from(index)
                        .map_err(|_| invalid("Engram sparse gather index exceeds u32"))?,
                );
            }
        }
        let ids = Tensor::new(row_indices.as_slice(), self.device.tensor_device())
            .map_err(tensor_error)?;
        let data = data
            .index_select(&ids, 0)
            .and_then(|t| t.reshape((tokens, PROJECTED_INPUT_WIDTH)))
            .map_err(tensor_error)?;
        let scales = scales
            .index_select(&ids, 0)
            .and_then(|t| t.reshape((tokens, PROJECTED_INPUT_WIDTH / 32)))
            .map_err(tensor_error)?;
        check_cancel(cancellation)?;
        let activations = Fp8Activations::from_checkpoint_rows(&self.device, &data, &scales)?;
        let image_mask = Tensor::new(image_mask.as_slice(), self.device.tensor_device())
            .map_err(tensor_error)?;
        check_cancel(cancellation)?;
        Ok(EngramEmbeddings {
            activations,
            image_mask,
            device: self.device.clone(),
            live_bytes,
        })
    }

    pub(crate) fn live_tensor_bytes(&self, tokens: usize) -> Result<u64> {
        self.device
            .limits()
            .checked_elements(&[tokens, PROJECTED_INPUT_WIDTH], "Engram sparse entries")?;
        if tokens
            .checked_mul(TABLES_PER_LAYER)
            .is_none_or(|n| n >= u32::MAX as usize)
        {
            return Err(invalid("Engram sparse row count exceeds u32"));
        }
        if tokens > self.device.limits().max_context_tokens {
            return Err(invalid("Engram lookup tokens exceed its context budget"));
        }
        // Hash rows, the bounded BTreeSet/unique vector, upload IDs and chunk
        // descriptors coexist. Admit host metadata before building the plan.
        let metadata_bytes = tokens
            .checked_mul(TABLES_PER_LAYER * 128)
            .and_then(|n| n.checked_add(4096))
            .ok_or_else(|| invalid("Engram sparse host metadata bytes overflowed"))?;
        if metadata_bytes > self.device.limits().max_input_bytes {
            return Err(invalid(
                "Engram sparse plan exceeds its input metadata budget",
            ));
        }
        let bytes = (tokens as u64)
            .checked_mul(PROJECTED_INPUT_WIDTH as u64)
            .and_then(|n| n.checked_mul(64))
            .and_then(|n| n.checked_add(16384))
            .ok_or_else(|| invalid("Engram sparse gather live budget overflowed"))?;
        self.device.limits().checked_state_bytes(
            bytes,
            "Engram sparse gather data/SF/metadata and validation",
        )
    }
}

pub struct EngramEmbeddings {
    activations: Fp8Activations,
    image_mask: Tensor,
    device: CudaLayerDevice,
    live_bytes: u64,
}

impl EngramEmbeddings {
    pub fn activations(&self) -> &Fp8Activations {
        &self.activations
    }
    pub fn image_mask(&self) -> &Tensor {
        &self.image_mask
    }
    pub(crate) fn device(&self) -> &CudaLayerDevice {
        &self.device
    }
    pub(crate) fn live_tensor_bytes(&self) -> u64 {
        self.live_bytes
    }
}

fn check_cancel(cancellation: &CancellationToken) -> Result<()> {
    if cancellation.is_cancelled() {
        Err(PowerError::InferenceCancelled)
    } else {
        Ok(())
    }
}

#[cfg(test)]
pub(crate) mod tests;
