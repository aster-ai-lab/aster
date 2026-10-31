//! Wire DTOs for directory admission between aster-api and aster-auth.
//!
//! These stay on the `client` feature surface so api can implement the
//! directory HTTP endpoints without pulling auth stores / argon2.

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::domain::{Admission, SubjectStatus};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DirectoryAdmitRequest {
    pub plane: String,
    pub identifier: String,
    #[serde(default)]
    pub org_id: Option<Uuid>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DirectoryDecision {
    pub subject_id: Option<Uuid>,
    pub org_id: Option<Uuid>,
    pub active: bool,
    pub console_login: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DirectoryStatusRequest {
    pub plane: String,
}

impl From<Admission> for DirectoryDecision {
    fn from(value: Admission) -> Self {
        Self {
            subject_id: value.subject_id,
            org_id: value.org_id,
            active: value.active,
            console_login: value.console_login,
        }
    }
}

impl From<SubjectStatus> for DirectoryDecision {
    fn from(value: SubjectStatus) -> Self {
        Self {
            subject_id: None,
            org_id: value.org_id,
            active: value.active,
            console_login: value.console_login,
        }
    }
}

/// Parse a directory HTTP JSON body into a decision (shared by client + tests).
/// aster-api wraps the decision in the product envelope; the fields live in `data`.
pub fn parse_directory_http_body(bytes: &[u8]) -> Result<DirectoryDecision, String> {
    let value: serde_json::Value = serde_json::from_slice(bytes).map_err(|err| err.to_string())?;
    if let Some(data) = value.get("data").filter(|data| data.is_object()) {
        return serde_json::from_value(data.clone()).map_err(|err| err.to_string());
    }
    serde_json::from_value(value).map_err(|err| err.to_string())
}
