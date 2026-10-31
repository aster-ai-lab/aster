//! Engram compression of the exact official tokenizer artifact.
//!
//! Rust adaptation of DeepInfer's `engram_metadata.py` (Apache-2.0), revision
//! 9bd9cf243fc53ef9afbf8f04d391d989117d6ab9. The tokenizer is immutable model
//! data, not a product configuration format. See licenses/DeepInfer-Apache-2.0.txt.

use std::collections::HashMap;

use aster_power::error::{PowerError, Result};
use sha2::{Digest, Sha256};
use tokenizers::normalizers::replace::ReplacePattern;
use tokenizers::normalizers::{Lowercase, Replace, Sequence, Strip, StripAccents, NFD, NFKC};
use tokenizers::{NormalizedString, Normalizer, Tokenizer};

pub const OFFICIAL_TOKENIZER_SHA256: &str =
    "c90dfa01249db1be4245780a052ede752e1361c612ac6d08e2bdada7d599476b";
pub const COMPRESSED_MAP_SHA256: &str =
    "c60a86322ec17b4142bfef3c57a8d81fb428550cdf487f88a4320cb59fe46481";
pub const RAW_VOCABULARY: usize = 129_280;
pub const COMPRESSED_VOCABULARY: u32 = 99_092;
pub const COMPRESSED_PAD: u32 = 2;
pub const MAX_TOKENIZER_BYTES: usize = 16 * 1024 * 1024;
const PAD_SPELLING: &str = "<｜▁pad▁｜>";

pub struct CompressedVocabulary {
    mapping: Vec<u32>,
}

impl CompressedVocabulary {
    /// The XQ header exports this exact official map. Its verified owner
    /// reads the U32 array before entering this crate-private binding.
    pub(crate) fn from_pinned_mapping(mapping: Vec<u32>) -> Result<Self> {
        if mapping.len() != RAW_VOCABULARY || mapping.iter().any(|id| *id >= COMPRESSED_VOCABULARY)
        {
            return Err(invalid(
                "Engram token map has an invalid length or compressed ID",
            ));
        }
        let vocabulary = Self { mapping };
        if vocabulary.compress(2)? != COMPRESSED_PAD
            || vocabulary.mapping_sha256() != COMPRESSED_MAP_SHA256
        {
            return Err(invalid(
                "Engram token map differs from the pinned official mapping",
            ));
        }
        Ok(vocabulary)
    }

    pub fn from_official_tokenizer(bytes: &[u8]) -> Result<Self> {
        if bytes.is_empty() || bytes.len() > MAX_TOKENIZER_BYTES {
            return Err(invalid("tokenizer artifact exceeds its byte bound"));
        }
        if format!("{:x}", Sha256::digest(bytes)) != OFFICIAL_TOKENIZER_SHA256 {
            return Err(invalid(
                "tokenizer SHA-256 differs from the pinned official revision",
            ));
        }
        let tokenizer =
            Tokenizer::from_bytes(bytes).map_err(|_| invalid("invalid tokenizer artifact"))?;
        if tokenizer.get_vocab_size(true) != RAW_VOCABULARY
            || tokenizer.token_to_id(PAD_SPELLING) != Some(2)
        {
            return Err(invalid(
                "tokenizer vocabulary or PAD entity differs from the official schema",
            ));
        }
        let (mapping, count) = build_map(&tokenizer, RAW_VOCABULARY)?;
        let vocabulary = Self { mapping };
        if count != COMPRESSED_VOCABULARY
            || vocabulary.compress(2)? != COMPRESSED_PAD
            || vocabulary.mapping_sha256() != COMPRESSED_MAP_SHA256
        {
            return Err(invalid(
                "Engram compressed vocabulary differs from the pinned reference",
            ));
        }
        Ok(vocabulary)
    }

    /// Image tokens are supplied explicitly by the multimodal owner. Unknown
    /// raw IDs are not guessed to be images or silently mapped to PAD.
    pub fn compress(&self, raw_id: u32) -> Result<u32> {
        self.mapping
            .get(raw_id as usize)
            .copied()
            .ok_or_else(|| invalid("token ID is outside the pinned tokenizer vocabulary"))
    }

    pub fn mapping_sha256(&self) -> String {
        let mut digest = Sha256::new();
        for value in &self.mapping {
            digest.update(value.to_le_bytes());
        }
        format!("{:x}", digest.finalize())
    }

    pub fn raw_vocabulary_size(&self) -> usize {
        self.mapping.len()
    }
}

fn compression_normalizer() -> Result<Sequence> {
    // These are the exact upstream byte/Unicode normalization rules, not
    // entity or intent matching. Preserve whitespace-only token distinctions.
    let replace = |pattern, content: &str| {
        Replace::new(pattern, content).map_err(|_| invalid("Engram normalizer construction failed"))
    };
    Ok(Sequence::new(vec![
        NFKC.into(),
        NFD.into(),
        StripAccents.into(),
        Lowercase.into(),
        replace(ReplacePattern::Regex("[ \\t\\r\\n]+".into()), " ")?.into(),
        replace(ReplacePattern::Regex("^ $".into()), "\u{e000}")?.into(),
        Strip::new(true, true).into(),
        replace(ReplacePattern::String("\u{e000}".into()), " ")?.into(),
    ]))
}

fn compression_key(
    decoded: &str,
    vocabulary_spelling: &str,
    normalizer: &Sequence,
) -> Result<String> {
    if decoded.contains('\u{fffd}') {
        return Ok(vocabulary_spelling.into());
    }
    let mut normalized = NormalizedString::from(decoded);
    normalizer
        .normalize(&mut normalized)
        .map_err(|_| invalid("Engram vocabulary normalization failed"))?;
    Ok(if normalized.get().is_empty() {
        decoded.into()
    } else {
        normalized.get().into()
    })
}

fn build_map(tokenizer: &Tokenizer, maximum_vocabulary: usize) -> Result<(Vec<u32>, u32)> {
    let count = tokenizer.get_vocab_size(true);
    if count == 0 || count > maximum_vocabulary || count > RAW_VOCABULARY {
        return Err(invalid(
            "compressed tokenizer vocabulary exceeds its nonzero bound",
        ));
    }
    let normalizer = compression_normalizer()?;
    let mut keys = HashMap::new();
    let mut mapping = Vec::with_capacity(count);
    for id in 0..count as u32 {
        let decoded = tokenizer
            .decode(&[id], false)
            .map_err(|_| invalid("tokenizer single-token decoding failed"))?;
        let spelling = tokenizer
            .id_to_token(id)
            .ok_or_else(|| invalid("tokenizer vocabulary has a missing token ID"))?;
        let key = compression_key(&decoded, &spelling, &normalizer)?;
        let next = keys.len() as u32;
        mapping.push(*keys.entry(key).or_insert(next));
    }
    Ok((mapping, keys.len() as u32))
}

fn invalid(message: &str) -> PowerError {
    PowerError::InvalidFormat(message.into())
}

#[cfg(test)]
mod tests;
