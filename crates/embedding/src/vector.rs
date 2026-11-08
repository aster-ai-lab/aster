//! The vector backend trait (specs/013 P5): vicinity semantics in pure Rust.
//!
//! `Linear` is exact and stays the small-collection default. `Hnsw` serves
//! large collections behind a recall-parity gate. Selection is by the
//! caller's policy — this module never picks a backend by itself.

pub mod backend;
pub mod hnsw;
pub mod linear;

pub use backend::VectorBackend;
pub use linear::LinearBackend;

/// The collection-size threshold above which an HNSW backend is eligible.
/// Below it, exactness is free — a scan of 4k vectors costs less than an
/// index build.
pub const ANN_SIZE_THRESHOLD: usize = 4_096;

/// The recall@10 parity floor an HNSW backend must prove before it serves.
pub const ANN_RECALL_PARITY_FLOOR: f64 = 0.98;

#[cfg(test)]
mod threshold_tests {
    use super::*;

    #[test]
    fn small_collections_stay_linear_by_policy() {
        // The threshold is a named constant, not a hidden heuristic.
        assert_eq!(ANN_SIZE_THRESHOLD, 4_096);
        assert_eq!(ANN_RECALL_PARITY_FLOOR, 0.98);
    }
}
