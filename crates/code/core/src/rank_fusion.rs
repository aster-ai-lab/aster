//! Core's shared rank/score accumulation. Callers own candidate budgets, provenance,
//! normalization and final ordering; this module performs no I/O or inference.

use std::collections::{hash_map::Entry, HashMap};
use std::num::NonZeroUsize;
use std::ops::{AddAssign, Div};

const RRF_K: usize = 60;

pub(crate) struct RankFusion<V, C, S> {
    entries: HashMap<String, Fused<V, C, S>>,
}

pub(crate) struct Fused<V, C, S> {
    value: V,
    score: S,
    ranks: Vec<(C, NonZeroUsize)>,
}

// Scalar width is part of the existing domain score contract.
pub(crate) trait FusionScore: Copy + AddAssign + Div<Output = Self> + From<u8> {
    fn from_usize(value: usize) -> Self;
}

impl FusionScore for f32 {
    fn from_usize(value: usize) -> Self {
        value as Self
    }
}

impl FusionScore for f64 {
    fn from_usize(value: usize) -> Self {
        value as Self
    }
}

impl<V, C: Copy + Eq, S: FusionScore> RankFusion<V, C, S> {
    pub(crate) fn new() -> Self {
        Self {
            entries: HashMap::new(),
        }
    }

    pub(crate) fn add(
        &mut self,
        id: String,
        channel: C,
        rank: usize,
        value: V,
        merge: impl FnOnce(&mut V, V),
    ) {
        self.add_with_score(id, channel, rank, None, value, merge);
    }

    /// `score` is admitted by the calling domain; `None` retains RRF k=60.
    pub(crate) fn add_with_score(
        &mut self,
        id: String,
        channel: C,
        rank: usize,
        score: Option<S>,
        value: V,
        merge: impl FnOnce(&mut V, V),
    ) {
        let Some(rank) = NonZeroUsize::new(rank) else {
            return;
        };
        let contribution =
            score.unwrap_or_else(|| S::from(1) / S::from_usize(RRF_K.saturating_add(rank.get())));
        match self.entries.entry(id) {
            Entry::Vacant(entry) => {
                entry.insert(Fused {
                    value,
                    score: contribution,
                    ranks: vec![(channel, rank)],
                });
            }
            Entry::Occupied(mut entry) => {
                let entry = entry.get_mut();
                if entry.rank(channel).is_some() {
                    return;
                }
                merge(&mut entry.value, value);
                entry.score += contribution;
                entry.ranks.push((channel, rank));
            }
        }
    }

    pub(crate) fn values(&self) -> impl Iterator<Item = &Fused<V, C, S>> {
        self.entries.values()
    }

    pub(crate) fn into_entries(self) -> impl Iterator<Item = (String, Fused<V, C, S>)> {
        self.entries.into_iter()
    }
}

impl<V, C: Copy + Eq, S: FusionScore> Fused<V, C, S> {
    pub(crate) fn score(&self) -> S {
        self.score
    }

    pub(crate) fn rank(&self, channel: C) -> Option<NonZeroUsize> {
        self.ranks
            .iter()
            .find_map(|(existing, rank)| (*existing == channel).then_some(*rank))
    }

    pub(crate) fn into_parts(self) -> (V, Vec<(C, NonZeroUsize)>) {
        (self.value, self.ranks)
    }
}

#[cfg(test)]
#[path = "rank_fusion_tests.rs"]
mod tests;
