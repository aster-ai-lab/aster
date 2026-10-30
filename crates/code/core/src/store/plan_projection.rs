use serde::{Deserialize, Serialize};

/// Read-only Plan projection. `revision` stays absent until Core persists a
/// real Plan revision; the digest only identifies this projected task set.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SessionPlanProjection {
    pub tasks: Vec<crate::planning::Task>,
    pub digest: String,
    pub revision: Option<u64>,
}
