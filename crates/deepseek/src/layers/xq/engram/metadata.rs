//! Exact exported Engram/token-map binding. Header arrays are authenticated
//! through Power before they may select the existing official sparse tables.

use aster_power::{error::Result, inference::InferenceLimits};
use tokio_util::sync::CancellationToken;

use super::{check_cancel, invalid};
use crate::{
    engram::{EngramLayer, NGRAM_SIZE, TABLES_PER_LAYER},
    layers::{engram::lookup::EngramTable, CudaLayerDevice},
    tokenizer::{CompressedVocabulary, COMPRESSED_PAD, COMPRESSED_VOCABULARY, RAW_VOCABULARY},
    weights::{
        xq::{gguf::XqMetadataValue, storage::XqWeights},
        CheckpointWeights, MatrixLayout,
    },
};

pub struct XqEngramMetadata {
    vocabulary: CompressedVocabulary,
}

impl XqEngramMetadata {
    pub fn load(
        weights: &XqWeights,
        limits: &InferenceLimits,
        cancellation: &CancellationToken,
    ) -> Result<Self> {
        check_cancel(cancellation)?;
        limits.validate()?;
        // Raw authenticated map + decoded map + hash/parameter arrays. No
        // unbounded metadata allocation or original-tokenizer reconstruction.
        let bytes = RAW_VOCABULARY * 8 + 4096;
        if bytes > limits.max_input_bytes {
            return Err(invalid(
                "XQ Engram metadata exceeds the input metadata budget",
            ));
        }
        limits.checked_state_bytes(bytes as u64, "XQ Engram authenticated metadata")?;
        let metadata = weights.header().metadata();
        if metadata.get("deepseek41.engram.encoding")
            != Some(&XqMetadataValue::String("e4m3_e8m0_32_row264".into()))
            || metadata.get("deepseek41.engram.compressed_vocab_size")
                != Some(&XqMetadataValue::Unsigned(u64::from(COMPRESSED_VOCABULARY)))
            || metadata.get("deepseek41.engram.pad_id")
                != Some(&XqMetadataValue::Unsigned(u64::from(COMPRESSED_PAD)))
        {
            return Err(invalid(
                "XQ Engram encoding/vocabulary/PAD differs from the supported schema",
            ));
        }
        let layer_ids = u32s(weights, "deepseek41.engram.layer_ids", 2, cancellation)?;
        let rows = u32s(weights, "deepseek41.engram.rows", 2, cancellation)?;
        let primes = u32s(
            weights,
            "deepseek41.engram.primes",
            2 * TABLES_PER_LAYER,
            cancellation,
        )?;
        let multipliers = weights.read_metadata_array(
            "deepseek41.engram.multipliers",
            10,
            2 * NGRAM_SIZE,
            cancellation,
        )?;
        let multipliers: Vec<_> = multipliers
            .bytes()
            .chunks_exact(8)
            .map(|b| u64::from_le_bytes([b[0], b[1], b[2], b[3], b[4], b[5], b[6], b[7]]))
            .collect();
        if layer_ids != [1, 14] {
            return Err(invalid(
                "XQ Engram layer IDs require the exact ordered set [1,14]",
            ));
        }
        for (slot, layer) in [EngramLayer::Layer1, EngramLayer::Layer14]
            .into_iter()
            .enumerate()
        {
            if rows[slot] != layer.table_rows()
                || &primes[slot * TABLES_PER_LAYER..(slot + 1) * TABLES_PER_LAYER] != layer.sizes()
                || &multipliers[slot * NGRAM_SIZE..(slot + 1) * NGRAM_SIZE] != layer.multipliers()
            {
                return Err(invalid(
                    "XQ Engram table extent/hash parameters differ from the exact official layer",
                ));
            }
        }
        let mapping = u32s(
            weights,
            "deepseek41.engram.token_map",
            RAW_VOCABULARY,
            cancellation,
        )?;
        let vocabulary = CompressedVocabulary::from_pinned_mapping(mapping)?;
        check_cancel(cancellation)?;
        Ok(Self { vocabulary })
    }

    pub fn vocabulary(&self) -> &CompressedVocabulary {
        &self.vocabulary
    }

    /// A verified official collection supplies the external table bytes.
    /// XQ's GGUF contains no giant table payload. Hash identities are checked
    /// above; shape, dtype, scale and collection hashes stay owned by Power.
    pub fn bind_official_table<'a>(
        &self,
        device: &CudaLayerDevice,
        checkpoint: &'a CheckpointWeights,
        layer: EngramLayer,
    ) -> Result<EngramTable<'a>> {
        self.bind_table_source(
            device,
            crate::weights::engram::XqEngramTableSource::Official(checkpoint),
            layer,
        )
    }

    pub(crate) fn bind_table_source<'a>(
        &self,
        device: &CudaLayerDevice,
        source: crate::weights::engram::XqEngramTableSource<'a>,
        layer: EngramLayer,
    ) -> Result<EngramTable<'a>> {
        source.bind(device, layer)
    }
}

fn u32s(
    weights: &XqWeights,
    name: &str,
    count: usize,
    cancellation: &CancellationToken,
) -> Result<Vec<u32>> {
    let raw = weights.read_metadata_array(name, 4, count, cancellation)?;
    Ok(raw
        .bytes()
        .chunks_exact(4)
        .map(|b| u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
        .collect())
}

#[cfg(test)]
pub(crate) mod tests;
