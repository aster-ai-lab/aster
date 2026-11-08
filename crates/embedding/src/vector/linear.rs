//! The exact linear backend — today's scan, now behind the trait.

use super::backend::{Neighbor, VectorBackend, VectorEntry, cosine_distance};

pub struct LinearBackend {
    entries: Vec<VectorEntry>,
}

impl LinearBackend {
    pub fn new() -> Self {
        Self {
            entries: Vec::new(),
        }
    }
}

impl Default for LinearBackend {
    fn default() -> Self {
        Self::new()
    }
}

impl VectorBackend for LinearBackend {
    fn insert(&mut self, entry: VectorEntry) {
        self.entries.push(entry);
    }

    fn remove(&mut self, id: &str) {
        self.entries.retain(|entry| entry.id != id);
    }

    fn query_k(&self, vector: &[f32], k: usize) -> Vec<Neighbor> {
        let mut scored: Vec<Neighbor> = self
            .entries
            .iter()
            .map(|entry| Neighbor {
                id: entry.id.clone(),
                distance: cosine_distance(vector, &entry.vector),
            })
            .collect();
        scored.sort_by(|a, b| {
            a.distance
                .partial_cmp(&b.distance)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        scored.truncate(k);
        scored
    }

    fn len(&self) -> usize {
        self.entries.len()
    }

    fn backend_name(&self) -> &'static str {
        "linear"
    }
}

#[cfg(test)]
mod linear_tests {
    use super::*;

    fn entry(id: &str, vector: Vec<f32>) -> VectorEntry {
        VectorEntry {
            id: id.into(),
            vector,
        }
    }

    #[test]
    fn exact_query_returns_sorted_neighbors() {
        let mut backend = LinearBackend::new();
        backend.insert(entry("near", vec![1.0, 0.0]));
        backend.insert(entry("far", vec![0.0, 1.0]));
        backend.insert(entry("mid", vec![0.9, 0.1]));
        let hits = backend.query_k(&[1.0, 0.0], 2);
        assert_eq!(hits.len(), 2);
        assert_eq!(hits[0].id, "near");
        assert_eq!(hits[1].id, "mid");
        assert!(hits[0].distance < hits[1].distance);
    }

    #[test]
    fn remove_actually_removes() {
        let mut backend = LinearBackend::new();
        backend.insert(entry("a", vec![1.0]));
        backend.insert(entry("b", vec![2.0]));
        backend.remove("a");
        assert_eq!(backend.len(), 1);
        assert_eq!(backend.query_k(&[1.0], 5)[0].id, "b");
    }

    #[test]
    fn cosine_distance_bounds() {
        assert!(cosine_distance(&[1.0, 0.0], &[1.0, 0.0]).abs() < 1e-6);
        assert!((cosine_distance(&[1.0, 0.0], &[0.0, 1.0]) - 1.0).abs() < 1e-6);
        assert_eq!(cosine_distance(&[], &[1.0]), 1.0);
        assert_eq!(cosine_distance(&[0.0], &[1.0]), 1.0);
    }
}
