//! Tokenizer wrapper shared by the BGE engines.
//!
//! The tokenizer file shipped with the model owns its post-processor
//! (XLM-R style `<s> A </s></s> B </s>` pairs); this wrapper only bounds
//! sequence length, pads batches, and reports token counts.

use std::path::Path;

use aster_use_core::UseResult;
use tokenizers::tokenizer::{TruncationDirection, TruncationParams, TruncationStrategy};

use crate::bge_error;

/// bge-m3 and bge-reranker-v2-m3 are both trained at an 8k context, but
/// CPU-serving platform defaults serve retrieval-sized inputs; 512 keeps
/// worst-case latency bounded on the shared engine.
pub(crate) const MAX_SEQUENCE_TOKENS: usize = 512;
const PAD_TOKEN: &str = "<pad>";

/// Encoded, right-padded batch: input ids, attention mask, total tokens.
pub(crate) type EncodedBatch = (Vec<Vec<i64>>, Vec<Vec<i64>>, u64);

pub(crate) struct PairTokenizer {
    tokenizer: tokenizers::Tokenizer,
    pad_id: i64,
}

impl PairTokenizer {
    pub(crate) fn load(path: &Path) -> UseResult<Self> {
        let mut tokenizer = tokenizers::Tokenizer::from_file(path).map_err(|error| {
            bge_error(
                "use.bge.tokenizer_invalid",
                format!("Failed to load the tokenizer '{}': {error}", path.display()),
            )
        })?;
        tokenizer
            .with_truncation(Some(TruncationParams {
                max_length: MAX_SEQUENCE_TOKENS,
                strategy: TruncationStrategy::LongestFirst,
                stride: 0,
                direction: TruncationDirection::Right,
            }))
            .map_err(|error| {
                bge_error(
                    "use.bge.tokenizer_invalid",
                    format!("Failed to configure tokenizer truncation: {error}"),
                )
            })?;
        let pad_id = tokenizer
            .token_to_id(PAD_TOKEN)
            .map(i64::from)
            .ok_or_else(|| {
                bge_error(
                    "use.bge.tokenizer_invalid",
                    format!("The tokenizer defines no '{PAD_TOKEN}' token."),
                )
            })?;
        Ok(Self { tokenizer, pad_id })
    }

    /// Encodes standalone texts into one right-padded batch.
    pub(crate) fn encode_batch(&self, texts: &[String]) -> UseResult<EncodedBatch> {
        let mut ids = Vec::with_capacity(texts.len());
        let mut mask = Vec::with_capacity(texts.len());
        let mut total_tokens = 0_u64;
        for text in texts {
            let encoding = self
                .tokenizer
                .encode(text.as_str(), true)
                .map_err(|error| {
                    bge_error(
                        "use.bge.tokenizer_invalid",
                        format!("Failed to encode an embedding input: {error}"),
                    )
                })?;
            total_tokens += encoding.get_ids().len() as u64;
            ids.push(encoding.get_ids().iter().map(|id| i64::from(*id)).collect());
            mask.push(
                encoding
                    .get_attention_mask()
                    .iter()
                    .map(|value| i64::from(*value))
                    .collect(),
            );
        }
        pad_to_uniform(&mut ids, &mut mask, self.pad_id);
        Ok((ids, mask, total_tokens))
    }

    /// Encodes one query paired with each document into a padded batch.
    pub(crate) fn encode_pairs(
        &self,
        query: &str,
        documents: &[String],
    ) -> UseResult<EncodedBatch> {
        let mut ids = Vec::with_capacity(documents.len());
        let mut mask = Vec::with_capacity(documents.len());
        let mut total_tokens = 0_u64;
        for document in documents {
            let encoding = self
                .tokenizer
                .encode((query, document.as_str()), true)
                .map_err(|error| {
                    bge_error(
                        "use.bge.tokenizer_invalid",
                        format!("Failed to encode a rerank pair: {error}"),
                    )
                })?;
            total_tokens += encoding.get_ids().len() as u64;
            ids.push(encoding.get_ids().iter().map(|id| i64::from(*id)).collect());
            mask.push(
                encoding
                    .get_attention_mask()
                    .iter()
                    .map(|value| i64::from(*value))
                    .collect(),
            );
        }
        pad_to_uniform(&mut ids, &mut mask, self.pad_id);
        Ok((ids, mask, total_tokens))
    }
}

fn pad_to_uniform(ids: &mut [Vec<i64>], mask: &mut [Vec<i64>], pad_id: i64) {
    let longest = ids.iter().map(Vec::len).max().unwrap_or(0);
    for (row_ids, row_mask) in ids.iter_mut().zip(mask.iter_mut()) {
        while row_ids.len() < longest {
            row_ids.push(pad_id);
            row_mask.push(0);
        }
    }
}
