//! Shared age, curation and quota decisions for existing memory stores.

use crate::{memory_is_prune_protected, MemoryItem, PrunePolicy, RelevanceConfig};
use chrono::{DateTime, Utc};

/// A refused retention input. No memory content is retained in the error.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MemoryRetentionError {
    field: &'static str,
    reason: &'static str,
}

impl MemoryRetentionError {
    /// The stable input field whose value was refused.
    pub fn field(&self) -> &'static str {
        self.field
    }
}

impl std::fmt::Display for MemoryRetentionError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "memory retention refuses {}: {}",
            self.field, self.reason
        )
    }
}

impl std::error::Error for MemoryRetentionError {}

/// A validated policy and one clock observation shared by both pruning phases.
///
/// Construction and selection perform no I/O or mutation. Stores retain their
/// existing snapshot, locking and deletion semantics.
#[derive(Clone, Debug)]
pub struct ValidatedPrunePolicy {
    now: DateTime<Utc>,
    cutoff: DateTime<Utc>,
    min_importance: f32,
    max_items: usize,
}

impl ValidatedPrunePolicy {
    /// Refuse nonfinite thresholds or an age outside the datetime range.
    pub fn try_new(policy: &PrunePolicy, now: DateTime<Utc>) -> Result<Self, MemoryRetentionError> {
        if !policy.min_importance_to_keep.is_finite() {
            return Err(MemoryRetentionError {
                field: "min_importance_to_keep",
                reason: "threshold must be finite",
            });
        }
        let cutoff = chrono::Duration::try_days(i64::from(policy.max_age_days))
            .and_then(|age| now.checked_sub_signed(age))
            .ok_or(MemoryRetentionError {
                field: "max_age_days",
                reason: "age exceeds the supported datetime range",
            })?;
        Ok(Self {
            now,
            cutoff,
            min_importance: policy.min_importance_to_keep,
            max_items: policy.max_items,
        })
    }

    /// Select IDs for deletion, refusing nonfinite record importance first.
    ///
    /// Protection is a hard exemption from age and quota. Equal relevance keeps
    /// input order. Work is linear, plus sorting when the quota is exceeded;
    /// callers own snapshot size and the blocking execution boundary.
    pub fn select(&self, items: &[MemoryItem]) -> Result<Vec<String>, MemoryRetentionError> {
        let mut delete_ids = Vec::new();
        let mut remaining = Vec::new();
        for item in items {
            if !item.importance.is_finite() {
                return Err(MemoryRetentionError {
                    field: "importance",
                    reason: "record importance must be finite",
                });
            }
            if !memory_is_prune_protected(item)
                && item.timestamp < self.cutoff
                && item.importance < self.min_importance
            {
                delete_ids.push(item.id.clone());
            } else {
                remaining.push(item);
            }
        }
        if self.max_items > 0 && remaining.len() > self.max_items {
            let protected = remaining
                .iter()
                .filter(|item| memory_is_prune_protected(item))
                .count();
            let keep = self.max_items.saturating_sub(protected);
            remaining.retain(|item| !memory_is_prune_protected(item));
            let relevance = RelevanceConfig::default();
            remaining.sort_by(|left, right| {
                right
                    .relevance_score_at(self.now, &relevance)
                    .partial_cmp(&left.relevance_score_at(self.now, &relevance))
                    .unwrap_or(std::cmp::Ordering::Equal)
            });
            delete_ids.extend(remaining.into_iter().skip(keep).map(|item| item.id.clone()));
        }
        Ok(delete_ids)
    }
}

#[cfg(test)]
#[path = "retention_tests.rs"]
mod tests;
