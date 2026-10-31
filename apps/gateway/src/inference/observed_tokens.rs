//! Shared observed-token lanes for admission reconciliation and usage spool.
//!
//! The response-body observer parses the upstream `usage` object into
//! [`ObservedTokens`] and records it into the atomic [`ObservedTokenUsage`]
//! cell shared with the usage lifecycle. The TPM limiter reconciles from the
//! resolved total while the terminal lifecycle event snapshots the full
//! split. `u64::MAX` is the unknown sentinel on every lane.

use std::sync::atomic::{AtomicU64, Ordering};

const UNKNOWN_TOKENS: u64 = u64::MAX;

/// Token split carried by `aster.gateway.usage-lifecycle.v2` terminal events.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, serde::Serialize)]
pub(crate) struct LifecycleTokenBreakdown {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) prompt_tokens: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) completion_tokens: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) cached_input_tokens: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) reasoning_tokens: Option<u64>,
}

impl LifecycleTokenBreakdown {
    /// At least one lane must be known for the split to carry information.
    pub(crate) fn is_empty(&self) -> bool {
        self.prompt_tokens.is_none()
            && self.completion_tokens.is_none()
            && self.cached_input_tokens.is_none()
            && self.reasoning_tokens.is_none()
    }
}

/// One parsed upstream `usage` observation. Unknown lanes stay `None`;
/// they are never invented as zero.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) struct ObservedTokens {
    pub(crate) prompt: Option<u64>,
    pub(crate) completion: Option<u64>,
    pub(crate) cached_input: Option<u64>,
    pub(crate) reasoning: Option<u64>,
    pub(crate) total: Option<u64>,
}

impl ObservedTokens {
    /// The billing-grade total: the upstream-reported total when present,
    /// otherwise the prompt/completion sum when both are known.
    pub(crate) fn resolved_total(&self) -> Option<u64> {
        self.total.or_else(|| match (self.prompt, self.completion) {
            (Some(prompt), Some(completion)) => Some(prompt.saturating_add(completion)),
            _ => None,
        })
    }
}

/// Atomic cell shared between the response-body observer and the usage
/// lifecycle. The observer records once the upstream `usage` is parsed; the
/// lifecycle snapshots the lanes when the response terminates.
#[derive(Debug)]
pub(crate) struct ObservedTokenUsage {
    prompt_tokens: AtomicU64,
    completion_tokens: AtomicU64,
    cached_input_tokens: AtomicU64,
    reasoning_tokens: AtomicU64,
    total_tokens: AtomicU64,
}

impl Default for ObservedTokenUsage {
    fn default() -> Self {
        Self::new()
    }
}

impl ObservedTokenUsage {
    pub(crate) fn new() -> Self {
        Self {
            prompt_tokens: AtomicU64::new(UNKNOWN_TOKENS),
            completion_tokens: AtomicU64::new(UNKNOWN_TOKENS),
            cached_input_tokens: AtomicU64::new(UNKNOWN_TOKENS),
            reasoning_tokens: AtomicU64::new(UNKNOWN_TOKENS),
            total_tokens: AtomicU64::new(UNKNOWN_TOKENS),
        }
    }

    /// Record one observation. The resolved total lands on the total lane so
    /// readers never re-derive arithmetic that the observer already settled.
    pub(crate) fn record(&self, usage: &ObservedTokens) {
        if let Some(prompt) = usage.prompt {
            self.prompt_tokens.store(prompt, Ordering::Release);
        }
        if let Some(completion) = usage.completion {
            self.completion_tokens.store(completion, Ordering::Release);
        }
        if let Some(cached) = usage.cached_input {
            self.cached_input_tokens.store(cached, Ordering::Release);
        }
        if let Some(reasoning) = usage.reasoning {
            self.reasoning_tokens.store(reasoning, Ordering::Release);
        }
        if let Some(total) = usage.resolved_total() {
            self.total_tokens.store(total, Ordering::Release);
        }
    }

    /// Resolved total for admission reconciliation; `None` before any
    /// observation.
    pub(crate) fn total(&self) -> Option<u64> {
        known(self.total_tokens.load(Ordering::Acquire))
    }

    /// Full split for the terminal lifecycle event; `None` when no lane was
    /// ever observed.
    pub(crate) fn breakdown(&self) -> Option<LifecycleTokenBreakdown> {
        let breakdown = LifecycleTokenBreakdown {
            prompt_tokens: known(self.prompt_tokens.load(Ordering::Acquire)),
            completion_tokens: known(self.completion_tokens.load(Ordering::Acquire)),
            cached_input_tokens: known(self.cached_input_tokens.load(Ordering::Acquire)),
            reasoning_tokens: known(self.reasoning_tokens.load(Ordering::Acquire)),
        };
        if breakdown.is_empty() {
            None
        } else {
            Some(breakdown)
        }
    }
}

fn known(value: u64) -> Option<u64> {
    if value == UNKNOWN_TOKENS {
        None
    } else {
        Some(value)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_cell_reports_no_total_or_breakdown() {
        let cell = ObservedTokenUsage::new();
        assert_eq!(cell.total(), None);
        assert_eq!(cell.breakdown(), None);
    }

    #[test]
    fn record_publishes_every_observed_lane() {
        let cell = ObservedTokenUsage::new();
        cell.record(&ObservedTokens {
            prompt: Some(11),
            completion: Some(4),
            cached_input: Some(6),
            reasoning: None,
            total: None,
        });
        assert_eq!(cell.total(), Some(15));
        assert_eq!(
            cell.breakdown(),
            Some(LifecycleTokenBreakdown {
                prompt_tokens: Some(11),
                completion_tokens: Some(4),
                cached_input_tokens: Some(6),
                reasoning_tokens: None,
            })
        );
    }

    #[test]
    fn upstream_total_wins_over_derived_sum() {
        let cell = ObservedTokenUsage::new();
        cell.record(&ObservedTokens {
            prompt: Some(3),
            completion: Some(7),
            total: Some(42),
            ..ObservedTokens::default()
        });
        assert_eq!(cell.total(), Some(42));
    }

    #[test]
    fn partial_observation_without_total_keeps_breakdown_but_no_total() {
        let cell = ObservedTokenUsage::new();
        cell.record(&ObservedTokens {
            prompt: Some(9),
            ..ObservedTokens::default()
        });
        assert_eq!(cell.total(), None);
        assert_eq!(
            cell.breakdown(),
            Some(LifecycleTokenBreakdown {
                prompt_tokens: Some(9),
                ..LifecycleTokenBreakdown::default()
            })
        );
    }

    #[test]
    fn empty_observation_records_nothing() {
        let cell = ObservedTokenUsage::new();
        cell.record(&ObservedTokens::default());
        assert_eq!(cell.total(), None);
        assert_eq!(cell.breakdown(), None);
    }
}
