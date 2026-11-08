//! The backend contract: insert, remove, and exact-or-approximate k-NN.

/// One vector's identity and value.
#[derive(Clone, Debug)]
pub struct VectorEntry {
    pub id: String,
    pub vector: Vec<f32>,
}

/// The result of one k-NN query: id and distance (cosine distance, lower
/// is closer).
#[derive(Clone, Debug, PartialEq)]
pub struct Neighbor {
    pub id: String,
    pub distance: f32,
}

/// A vector backend. Implementations must be deterministic for the same
/// insertion order; a recall-parity gate proves the approximate tier before
/// it serves.
pub trait VectorBackend: Send + Sync {
    fn insert(&mut self, entry: VectorEntry);
    fn remove(&mut self, id: &str);
    fn query_k(&self, vector: &[f32], k: usize) -> Vec<Neighbor>;
    fn len(&self) -> usize;
    fn is_empty(&self) -> bool {
        self.len() == 0
    }
    fn backend_name(&self) -> &'static str;
}

/// Cosine distance: 1 - cosine similarity. Two identical unit vectors have
/// distance 0; two orthogonal vectors have distance 1.
pub fn cosine_distance(a: &[f32], b: &[f32]) -> f32 {
    if a.len() != b.len() || a.is_empty() {
        return 1.0;
    }
    let dot: f32 = a.iter().zip(b).map(|(x, y)| x * y).sum();
    let mag_a: f32 = a.iter().map(|v| v * v).sum::<f32>().sqrt();
    let mag_b: f32 = b.iter().map(|v| v * v).sum::<f32>().sqrt();
    if mag_a == 0.0 || mag_b == 0.0 {
        return 1.0;
    }
    1.0 - dot / (mag_a * mag_b)
}
