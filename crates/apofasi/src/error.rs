//! Error types for Apofasi core operations.

use thiserror::Error;

/// Convenient result alias.
pub type Result<T> = std::result::Result<T, Error>;

/// Apofasi errors. Callers should treat these as actionable, not panics.
#[derive(Debug, Error)]
pub enum Error {
    /// Question schema is incomplete or inconsistent.
    #[error("invalid question `{id}`: {reason}")]
    InvalidQuestion {
        /// Question id.
        id: String,
        /// Human-readable reason.
        reason: String,
    },

    /// Sequence packing exhausted the option token budget.
    #[error("question `{id}` options exceed head_max_len={head_max_len}")]
    HeadBudgetExceeded {
        /// Question id.
        id: String,
        /// Configured head budget.
        head_max_len: usize,
    },

    /// Unknown checkpoint name or alias.
    #[error("unknown checkpoint `{name}`")]
    UnknownCheckpoint {
        /// Provided name.
        name: String,
    },

    /// State exceeds the configured sequence budget and truncation is refused.
    #[error("question `{id}` state exceeds max_len={max_len}")]
    StateOverflow {
        /// Question whose state exceeded the budget.
        id: String,
        /// Configured total sequence budget in tokens.
        max_len: usize,
    },

    /// JSON state serialization failure.
    #[error("state serialization failed: {0}")]
    StateSerialize(#[from] serde_json::Error),

    /// Checkpoint directory or files are missing / incompatible.
    #[error("checkpoint error: {0}")]
    Checkpoint(String),

    /// Neural inference backend failure (feature `infer`).
    #[error("inference error: {0}")]
    Infer(String),
}
