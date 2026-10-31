//! Exact Engram hashes and request-local transactional n-gram history.
//! Hash math follows TileKernels (MIT); current-first/image-boundary gathering
//! follows DeepInfer (Apache-2.0). Source identities are retained with the crate.

use std::sync::atomic::{AtomicU64, Ordering};

use aster_power::error::{PowerError, Result};

use crate::model::MAX_CONTEXT_TOKENS;
use crate::tokenizer::{CompressedVocabulary, COMPRESSED_PAD, COMPRESSED_VOCABULARY};

mod official;

pub const NGRAM_SIZE: usize = 4;
pub const TABLES_PER_LAYER: usize = 24;
static NEXT_HISTORY: AtomicU64 = AtomicU64::new(1);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EngramLayer {
    Layer1,
    Layer14,
}

impl EngramLayer {
    pub fn parse(value: &str) -> Result<Self> {
        match value {
            "1" => Ok(Self::Layer1),
            "14" => Ok(Self::Layer14),
            _ => Err(invalid("unsupported Engram layer; supported set: 1, 14")),
        }
    }
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Layer1 => "1",
            Self::Layer14 => "14",
        }
    }
    pub fn table_rows(self) -> u32 {
        self.sizes().iter().sum()
    }
    pub(crate) fn sizes(self) -> &'static [u32; TABLES_PER_LAYER] {
        match self {
            Self::Layer1 => &official::LAYER1_SIZES,
            Self::Layer14 => &official::LAYER14_SIZES,
        }
    }
    pub(crate) fn multipliers(self) -> &'static [u64; NGRAM_SIZE] {
        match self {
            Self::Layer1 => &official::LAYER1_MULTIPLIERS,
            Self::Layer14 => &official::LAYER14_MULTIPLIERS,
        }
    }

    pub fn hash(self, row: &NgramRow) -> Result<[i32; TABLES_PER_LAYER]> {
        if row
            .tokens
            .iter()
            .any(|token| *token >= COMPRESSED_VOCABULARY)
        {
            return Err(invalid(
                "Engram hash token is outside the compressed vocabulary",
            ));
        }
        if row.is_image {
            return Ok([-1; TABLES_PER_LAYER]);
        }
        let multipliers = self.multipliers();
        let products = row
            .tokens
            .iter()
            .zip(multipliers)
            .map(|(token, multiplier)| {
                u64::from(*token)
                    .checked_mul(*multiplier)
                    .filter(|value| *value <= i64::MAX as u64)
                    .ok_or_else(|| invalid("Engram hash product exceeds signed int64"))
            })
            .collect::<Result<Vec<_>>>()?;
        let mut result = [0; TABLES_PER_LAYER];
        let mut hash = products[0];
        let mut offset = 0_u32;
        for ngram in 0..NGRAM_SIZE - 1 {
            hash ^= products[ngram + 1];
            for head in 0..8 {
                let table = ngram * 8 + head;
                let size = self.sizes()[table];
                result[table] = (offset + (hash % u64::from(size)) as u32) as i32;
                offset = offset
                    .checked_add(size)
                    .ok_or_else(|| invalid("Engram table offset overflowed"))?;
            }
        }
        Ok(result)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EngramToken {
    compressed: Option<u32>,
}

impl EngramToken {
    pub fn from_raw(id: u32, vocabulary: &CompressedVocabulary) -> Result<Self> {
        Ok(Self {
            compressed: Some(vocabulary.compress(id)?),
        })
    }
    pub fn image() -> Self {
        Self { compressed: None }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NgramRow {
    tokens: [u32; NGRAM_SIZE],
    is_image: bool,
}

impl NgramRow {
    pub fn tokens(&self) -> &[u32; NGRAM_SIZE] {
        &self.tokens
    }
    pub fn is_image(&self) -> bool {
        self.is_image
    }
}

pub struct EngramHistory {
    owner: u64,
    generation: u64,
    position: usize,
    previous: [Option<u32>; NGRAM_SIZE - 1],
    context_limit: usize,
    step_limit: usize,
}

impl EngramHistory {
    pub fn new(context_limit: usize, step_limit: usize) -> Result<Self> {
        if context_limit == 0
            || context_limit > MAX_CONTEXT_TOKENS
            || step_limit == 0
            || step_limit > context_limit
        {
            return Err(invalid(
                "Engram history requires bounded positive context and step limits",
            ));
        }
        let owner = NEXT_HISTORY
            .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |value| {
                value.checked_add(1)
            })
            .map_err(|_| invalid("Engram history owner sequence exhausted"))?;
        Ok(Self {
            owner,
            generation: 0,
            position: 0,
            previous: [Some(COMPRESSED_PAD); NGRAM_SIZE - 1],
            context_limit,
            step_limit,
        })
    }

    pub fn position(&self) -> usize {
        self.position
    }

    /// Preparing or dropping a speculative/chunk suffix never mutates committed
    /// history. Images terminate every n-gram that crosses their boundary.
    pub fn prepare(&self, tokens: &[EngramToken]) -> Result<PreparedNgrams> {
        let end = self
            .position
            .checked_add(tokens.len())
            .ok_or_else(|| invalid("Engram context position overflowed"))?;
        if tokens.is_empty() || tokens.len() > self.step_limit || end > self.context_limit {
            return Err(invalid(
                "Engram step exceeds its nonzero token/context budget",
            ));
        }
        let mut previous = self.previous;
        let mut rows = Vec::with_capacity(tokens.len());
        for token in tokens {
            if token
                .compressed
                .is_some_and(|id| id >= COMPRESSED_VOCABULARY)
            {
                return Err(invalid(
                    "Engram history token is outside the compressed vocabulary",
                ));
            }
            let mut row = [COMPRESSED_PAD; NGRAM_SIZE];
            for (index, value) in std::iter::once(token.compressed)
                .chain(previous)
                .enumerate()
            {
                match value {
                    Some(value) => row[index] = value,
                    None => break,
                }
            }
            rows.push(NgramRow {
                tokens: row,
                is_image: token.compressed.is_none(),
            });
            advance(&mut previous, token.compressed);
        }
        Ok(PreparedNgrams {
            owner: self.owner,
            generation: self.generation,
            position: self.position,
            tokens: tokens.to_vec(),
            rows,
        })
    }

    /// Only an accepted prefix is committed. Stale or different-request
    /// transitions refuse before changing any position or history slot.
    pub fn commit_prefix(&mut self, prepared: PreparedNgrams, accepted: usize) -> Result<()> {
        self.stage_prefix(prepared, accepted)?.apply();
        Ok(())
    }

    pub(crate) fn stage_prefix(
        &mut self,
        prepared: PreparedNgrams,
        accepted: usize,
    ) -> Result<EngramCommit<'_>> {
        if prepared.owner != self.owner
            || prepared.generation != self.generation
            || prepared.position != self.position
            || accepted > prepared.tokens.len()
        {
            return Err(invalid("Engram transition belongs to another request/generation or has an invalid accepted prefix"));
        }
        if accepted == 0 {
            let previous = self.previous;
            let position = self.position;
            let generation = self.generation;
            return Ok(EngramCommit {
                state: self,
                previous,
                position,
                generation,
            });
        }
        let generation = self
            .generation
            .checked_add(1)
            .ok_or_else(|| invalid("Engram history generation exhausted"))?;
        let position = self
            .position
            .checked_add(accepted)
            .filter(|position| *position <= self.context_limit)
            .ok_or_else(|| invalid("Engram accepted prefix exceeds context capacity"))?;
        let mut previous = self.previous;
        for token in prepared.tokens.iter().take(accepted) {
            advance(&mut previous, token.compressed);
        }
        Ok(EngramCommit {
            state: self,
            previous,
            position,
            generation,
        })
    }
}

pub struct PreparedNgrams {
    owner: u64,
    generation: u64,
    position: usize,
    tokens: Vec<EngramToken>,
    rows: Vec<NgramRow>,
}

impl PreparedNgrams {
    pub(crate) fn position(&self) -> usize {
        self.position
    }
    pub(crate) fn token_count(&self) -> usize {
        self.tokens.len()
    }
    pub fn rows(&self) -> &[NgramRow] {
        &self.rows
    }
}

pub(crate) struct EngramCommit<'a> {
    state: &'a mut EngramHistory,
    previous: [Option<u32>; NGRAM_SIZE - 1],
    position: usize,
    generation: u64,
}

impl EngramCommit<'_> {
    pub(crate) fn apply(&mut self) {
        self.state.previous = self.previous;
        self.state.position = self.position;
        self.state.generation = self.generation;
    }
}

fn advance(previous: &mut [Option<u32>; NGRAM_SIZE - 1], token: Option<u32>) {
    previous.rotate_right(1);
    previous[0] = token;
}

fn invalid(message: &str) -> PowerError {
    PowerError::InvalidRequest(message.into())
}

#[cfg(test)]
pub(crate) mod tests;
