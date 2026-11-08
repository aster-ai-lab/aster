//! Pooling and activation math shared by the engines. Always compiled so
//! the numeric contract is testable without engine features.

/// In-place L2 normalization: unit length, or the zero vector unchanged
/// for a degenerate row.
pub(crate) fn l2_normalize(vector: &mut [f32]) {
    let square_sum: f32 = vector.iter().map(|value| value * value).sum();
    let norm = square_sum.sqrt();
    if norm > 0.0 && norm.is_finite() {
        for value in vector.iter_mut() {
            *value /= norm;
        }
    }
}

/// Maps a cross-encoder logit to a (0, 1) relevance score. The exp runs in
/// f64 to keep extreme logits finite in f32.
pub(crate) fn sigmoid(logit: f32) -> f32 {
    1.0 / (1.0 + (-f64::from(logit)).exp() as f32)
}
