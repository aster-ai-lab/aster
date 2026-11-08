//! The approximate HNSW tier behind the same trait (specs/013 P5).
//!
//! Built on `hnsw_rs` (pure Rust). The graph builds eagerly on flush;
//! removals rebuild (HNSW has no incremental delete). Recall parity
//! against the exact Linear backend is proven on a fixed fixture —
//! below parity, the caller's policy keeps Linear.

use hnsw_rs::prelude::*;

use super::backend::{Neighbor, VectorBackend, VectorEntry};

const MAX_LAYER: usize = 16;
const NB_CONNECTIONS: usize = 8;
const DEFAULT_EF: usize = 48;
const FLUSH_BATCH: usize = 1_024;

pub struct HnswBackend {
    index: Option<Hnsw<'static, f32, DistCosine>>,
    pending: Vec<VectorEntry>,
    entries: Vec<VectorEntry>,
    dimension: usize,
    ef: usize,
}

impl HnswBackend {
    pub fn new(dimension: usize) -> Self {
        Self {
            index: None,
            pending: Vec::new(),
            entries: Vec::new(),
            dimension,
            ef: DEFAULT_EF,
        }
    }

    fn flush(&mut self) {
        if self.pending.is_empty() {
            return;
        }
        let capacity = self.entries.len();
        let mut index =
            Hnsw::<f32, DistCosine>::new(NB_CONNECTIONS, capacity, MAX_LAYER, self.ef, DistCosine);
        for (slot, entry) in self.entries.iter().enumerate() {
            index.insert((&entry.vector, slot));
        }
        self.index = Some(index);
        self.pending.clear();
    }
}

impl VectorBackend for HnswBackend {
    fn insert(&mut self, entry: VectorEntry) {
        self.entries.push(entry.clone());
        self.pending.push(entry);
        if self.pending.len() >= FLUSH_BATCH {
            self.flush();
        }
    }

    fn remove(&mut self, id: &str) {
        self.entries.retain(|entry| entry.id != id);
        self.pending.retain(|entry| entry.id != id);
        // HNSW has no incremental delete: rebuild from the retained set.
        if !self.entries.is_empty() {
            let entries = self.entries.clone();
            self.pending = entries;
            self.flush();
        } else {
            self.index = None;
        }
    }

    fn query_k(&self, vector: &[f32], k: usize) -> Vec<Neighbor> {
        let Some(index) = &self.index else {
            return Vec::new();
        };
        index
            .search(vector, k, self.ef)
            .into_iter()
            .filter_map(|n| {
                let entry = self.entries.get(n.d_id)?;
                Some(Neighbor {
                    id: entry.id.clone(),
                    distance: n.distance,
                })
            })
            .collect()
    }

    fn len(&self) -> usize {
        self.entries.len()
    }

    fn backend_name(&self) -> &'static str {
        "hnsw"
    }
}

/// Recall@k parity: the fraction of the exact backend's top-k ids the
/// approximate backend also returns for the query.
pub fn recall_at_k(approx: &[Neighbor], exact: &[Neighbor]) -> f64 {
    if exact.is_empty() {
        return 0.0;
    }
    let exact_ids: std::collections::HashSet<&str> = exact.iter().map(|n| n.id.as_str()).collect();
    let hits = approx
        .iter()
        .filter(|n| exact_ids.contains(n.id.as_str()))
        .count();
    hits as f64 / exact.len() as f64
}

#[cfg(test)]
#[path = "hnsw_tests.rs"]
mod hnsw_tests;
