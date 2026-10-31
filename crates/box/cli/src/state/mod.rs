//! State management for box instances.
//!
//! Persists box metadata to `~/.aster/boxes.json` with atomic writes.
//! On every load, dead active PIDs are reconciled to mark boxes as dead.

mod file;
pub(crate) mod policy;
#[cfg(test)]
mod tests;

pub use aster_box_runtime::{BoxRecord, HealthCheck};
pub use file::StateFile;
pub use policy::{generate_name, parse_restart_policy, validate_restart_policy};
