use super::super::backend::VectorEntry;
use super::super::linear::LinearBackend;
use super::*;

fn circle_entry(i: usize, total: usize) -> VectorEntry {
    let angle = 2.0 * std::f32::consts::PI * i as f32 / total as f32;
    VectorEntry {
        id: format!("p{i}"),
        vector: vec![angle.cos(), angle.sin()],
    }
}

/// The named parity gate: HNSW must return the same top-5 as Linear
/// on the fixed 64-point circle fixture.
#[test]
fn hnsw_matches_linear_on_the_circle_fixture() {
    let total = 64;
    let mut exact = LinearBackend::new();
    let mut approx = HnswBackend::new(2);
    for i in 0..total {
        let entry = circle_entry(i, total);
        exact.insert(entry.clone());
        approx.insert(entry);
    }
    approx.flush();

    let query = [0.99875_f32.cos(), 0.99875_f32.sin()];
    let exact_hits = exact.query_k(&query, 5);
    let approx_hits = approx.query_k(&query, 5);
    let recall = recall_at_k(&approx_hits, &exact_hits);
    assert!(
        recall >= super::super::ANN_RECALL_PARITY_FLOOR,
        "recall {recall} below the parity floor"
    );
}

#[test]
fn removal_rebuilds_the_graph() {
    let mut backend = HnswBackend::new(2);
    backend.insert(circle_entry(0, 4));
    backend.insert(circle_entry(1, 4));
    backend.insert(circle_entry(2, 4));
    backend.flush();
    assert_eq!(backend.len(), 3);
    backend.remove("p1");
    assert_eq!(backend.len(), 2);
    // The remaining entries still answer.
    let hits = backend.query_k(&[1.0, 0.0], 2);
    assert_eq!(hits.len(), 2);
}
