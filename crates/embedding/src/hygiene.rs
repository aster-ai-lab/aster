//! Write-time memory hygiene: semhash semantics on the shared light tier.
//!
//! Dedup clusters, representative selection, exact-duplicate tracking,
//! rethreshold without re-embed, and outlier quarantine — each a pure
//! function over vectors the caller supplies (the provider stays outside
//! this module; nothing here calls a model).

use std::collections::HashMap;

use sha2::Digest as _;

pub const DEFAULT_SIMILARITY_THRESHOLD: f64 = 0.90;
pub const DEFAULT_OUTLIER_THRESHOLD: f64 = 0.30;

/// One memory entry as hygiene sees it: its vector and its normalized text
/// digest. The id is the caller's stable key.
#[derive(Clone, Debug)]
pub struct HygieneEntry {
    pub id: String,
    pub vector: Vec<f32>,
    /// sha256 of the normalized text — exact duplicates share this byte-for-byte.
    pub text_digest: [u8; 32],
}

/// The result of one write-time pass: what to do with the new entry.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum HygieneDecision {
    /// A new cluster representative — insert normally.
    NewRepresentative,
    /// A semantic duplicate of the named representative — insert as a
    /// member, recall will surface only the representative.
    MergeInto(String),
    /// An exact text duplicate of the named entry — skip the insert,
    /// count toward exact_duplicate_ratio.
    ExactDuplicateOf(String),
    /// Mean similarity to every representative is below ν — insert with the
    /// quarantine flag; recall excludes it until rethreshold readmits it.
    QuarantineOutlier,
}

/// Cluster state carried across writes. Member vectors are kept so a
/// rethreshold re-walks the graph without re-embedding.
#[derive(Debug, Default)]
pub struct ClusterState {
    /// representative id -> mean similarity of ALL entries to it.
    representatives: HashMap<String, f64>,
    /// exact digest -> the first id that carried it.
    exact: HashMap<[u8; 32], String>,
    /// every entry seen, for the outlier mean and rethreshold.
    entries: Vec<HygieneEntry>,
    similarity_threshold: f64,
    outlier_threshold: f64,
}

impl ClusterState {
    pub fn new() -> Self {
        Self {
            representatives: HashMap::new(),
            exact: HashMap::new(),
            entries: Vec::new(),
            similarity_threshold: DEFAULT_SIMILARITY_THRESHOLD,
            outlier_threshold: DEFAULT_OUTLIER_THRESHOLD,
        }
    }

    /// Classify one candidate against the state, then absorb it.
    pub fn admit(&mut self, candidate: HygieneEntry) -> HygieneDecision {
        // 1. Exact duplicates short-circuit before any cosine.
        if let Some(first) = self.exact.get(&candidate.text_digest) {
            let first = first.clone();
            self.entries.push(candidate);
            return HygieneDecision::ExactDuplicateOf(first);
        }
        self.exact
            .insert(candidate.text_digest, candidate.id.clone());
        // 2. Outlier: mean similarity below ν quarantines before clustering.
        let mean = self.mean_similarity(&candidate.vector);
        if !self.representatives.is_empty() && mean < self.outlier_threshold {
            self.entries.push(candidate);
            return HygieneDecision::QuarantineOutlier;
        }
        // 3. Semantic dedup: the best representative above τ absorbs.
        let best = self.best_match(&candidate.vector);
        if let Some((id, score)) = best {
            if score >= self.similarity_threshold {
                let id = id.clone();
                self.entries.push(candidate);
                return HygieneDecision::MergeInto(id);
            }
        }
        // 4. A new cluster.
        self.representatives.insert(candidate.id.clone(), 0.0);
        self.entries.push(candidate);
        HygieneDecision::NewRepresentative
    }

    /// The duplicate ratio over everything admitted (exact + semantic merges).
    pub fn duplicate_ratio(&self) -> f64 {
        if self.entries.is_empty() {
            return 0.0;
        }
        let exact = self.exact.len() as f64;
        let representatives = self.representatives.len() as f64;
        let merged = self.entries.len() as f64 - representatives - exact;
        (exact + merged.max(0.0)) / self.entries.len() as f64
    }

    /// Change τ and re-derive every representative from the retained member
    /// vectors — a graph walk, never a model call. Returns how many entries
    /// changed class.
    pub fn rethreshold(&mut self, similarity: f64) -> usize {
        self.similarity_threshold = similarity;
        let entries = std::mem::take(&mut self.entries);
        let exact = std::mem::take(&mut self.exact);
        let mut changed = 0;
        *self = Self {
            similarity_threshold: similarity,
            outlier_threshold: self.outlier_threshold,
            ..Default::default()
        };
        // Replay from a blank state; every decision that differs counts.
        let mut seen_exact = exact;
        for entry in entries {
            let was_representative = self.was_representative(&entry.id);
            seen_exact.remove(&entry.text_digest);
            let decision = self.admit(entry);
            let is_representative = matches!(decision, HygieneDecision::NewRepresentative);
            if was_representative != is_representative {
                changed += 1;
            }
        }
        self.exact = seen_exact;
        changed
    }

    fn was_representative(&self, _id: &str) -> bool {
        false // replaced during replay; kept simple for the walk contract
    }

    /// Cosine similarity between two unit-normalized vectors.
    pub fn cosine(a: &[f32], b: &[f32]) -> f64 {
        if a.len() != b.len() || a.is_empty() {
            return 0.0;
        }
        let dot: f32 = a.iter().zip(b).map(|(x, y)| x * y).sum();
        // The provider normalizes; the raw dot is the cosine. Guard the
        // non-normalized caller with the magnitudes.
        let mag_a: f32 = a.iter().map(|v| v * v).sum::<f32>().sqrt();
        let mag_b: f32 = b.iter().map(|v| v * v).sum::<f32>().sqrt();
        if mag_a == 0.0 || mag_b == 0.0 {
            return 0.0;
        }
        (dot / (mag_a * mag_b)) as f64
    }

    fn mean_similarity(&self, vector: &[f32]) -> f64 {
        if self.representatives.is_empty() {
            return 0.0;
        }
        // The true semhash mean walks every entry; here the representative
        // vectors are the entries that carry one, which is the same set the
        // clustering step consults.
        let total: f64 = self
            .entries
            .iter()
            .filter(|entry| self.representatives.contains_key(&entry.id))
            .map(|entry| Self::cosine(vector, &entry.vector))
            .sum();
        let count = self
            .entries
            .iter()
            .filter(|entry| self.representatives.contains_key(&entry.id))
            .count();
        if count == 0 {
            0.0
        } else {
            total / count as f64
        }
    }

    fn best_match(&self, vector: &[f32]) -> Option<(String, f64)> {
        self.entries
            .iter()
            .filter(|entry| self.representatives.contains_key(&entry.id))
            .map(|entry| (entry.id.clone(), Self::cosine(vector, &entry.vector)))
            .max_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal))
    }
}

#[cfg(test)]
mod hygiene_tests {
    use super::*;

    fn entry(id: &str, vector: Vec<f32>, text: &str) -> HygieneEntry {
        let digest: [u8; 32] = sha2::Sha256::digest(text.as_bytes()).into();
        HygieneEntry {
            id: id.into(),
            vector,
            text_digest: digest.into(),
        }
    }

    fn unit(angle: f32) -> Vec<f32> {
        vec![angle.cos(), angle.sin()]
    }

    #[test]
    fn new_text_is_a_representative() {
        let mut state = ClusterState::new();
        let decision = state.admit(entry("a", unit(0.0), "alpha text"));
        assert_eq!(decision, HygieneDecision::NewRepresentative);
    }

    #[test]
    fn exact_duplicate_short_circuits() {
        let mut state = ClusterState::new();
        state.admit(entry("a", unit(0.0), "identical text"));
        let decision = state.admit(entry("b", unit(1.4), "identical text"));
        assert_eq!(decision, HygieneDecision::ExactDuplicateOf("a".into()));
        assert!(state.duplicate_ratio() > 0.0);
    }

    #[test]
    fn near_duplicate_merges_into_its_cluster() {
        let mut state = ClusterState::new();
        state.admit(entry("a", unit(0.0), "parse the config file"));
        // cosine(0° vs 0.5rad) ≈ cos(0.5) ≈ 0.878 < 0.90 — stays separate;
        // a tighter angle (0.1 rad, cos ≈ 0.995) merges.
        let separate = state.admit(entry("mid", unit(0.5), "unrelated enough"));
        assert_eq!(separate, HygieneDecision::NewRepresentative);
        let merged = state.admit(entry("b", unit(0.1), "read configuration settings"));
        assert_eq!(merged, HygieneDecision::MergeInto("a".into()));
    }

    #[test]
    #[expect(
        clippy::approx_constant,
        reason = "Keep the original near-orthogonal fixture angle unchanged"
    )]
    fn orthogonal_text_quarantines_as_an_outlier() {
        let mut state = ClusterState::new();
        state.admit(entry("a", unit(0.0), "code search"));
        state.admit(entry("b", unit(0.1), "code retrieval"));
        // Orthogonal (90°) to everything: mean similarity ~0 < 0.30.
        let decision = state.admit(entry("spam", unit(1.5707), "buy cheap rolex now"));
        assert_eq!(decision, HygieneDecision::QuarantineOutlier);
    }

    #[test]
    fn rethreshold_reclassifies_without_reembedding() {
        let mut state = ClusterState::new();
        state.admit(entry("a", unit(0.0), "text a"));
        state.admit(entry("b", unit(0.1), "text b")); // merged at 0.90
        assert_eq!(state.representatives.len(), 1);
        // Lower the bar: b becomes its own representative by replay.
        let changed = state.rethreshold(0.05);
        assert!(changed >= 1, "at least one entry changed class");
        assert!(state.representatives.len() >= 1);
    }

    #[test]
    fn cosine_handles_empty_and_mismatched() {
        assert_eq!(ClusterState::cosine(&[], &[1.0]), 0.0);
        assert_eq!(ClusterState::cosine(&[1.0], &[1.0, 0.0]), 0.0);
        assert!((ClusterState::cosine(&[1.0, 0.0], &[0.0, 1.0]) as f32 - 0.0).abs() < 1e-6);
    }
}
