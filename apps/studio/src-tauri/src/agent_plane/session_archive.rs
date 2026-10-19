use std::collections::{HashMap, HashSet};
use std::sync::Mutex;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::{domain::new_id, error::PlaneError};

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct ArchiveRequest {
    pub session_ids: Vec<String>,
    pub operation_id: Option<String>,
    pub epoch: Option<String>,
}

pub(super) fn operation_uuid(value: &str) -> Result<String, PlaneError> {
    uuid::Uuid::parse_str(value)
        .map(|id| id.to_string())
        .map_err(|_| PlaneError::bad("archive operation identity must be a UUID"))
}

impl ArchiveRequest {
    pub fn parse(body: &Value) -> Result<Self, PlaneError> {
        let mut request: Self = serde_json::from_value(body.clone())
            .map_err(|error| PlaneError::bad(format!("invalid archive request: {error}")))?;
        if request.session_ids.is_empty()
            || request.session_ids.len() > 200
            || request.session_ids.iter().any(|id| id.trim().is_empty())
        {
            return Err(PlaneError::bad(
                "archive sessionIds must contain 1 to 200 nonempty IDs",
            ));
        }
        if request.operation_id.is_some() != request.epoch.is_some() {
            return Err(PlaneError::bad(
                "archive operationId and epoch are required together",
            ));
        }
        request.operation_id = request
            .operation_id
            .as_deref()
            .map(operation_uuid)
            .transpose()?;
        request.epoch = request.epoch.as_deref().map(operation_uuid).transpose()?;
        let mut seen = HashSet::new();
        request.session_ids.retain(|id| seen.insert(id.clone()));
        Ok(request)
    }
}

#[derive(Clone, Copy, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub(super) enum ArchiveStatus {
    Archived,
    AlreadyArchived,
    NotFound,
    Failed,
}

impl ArchiveStatus {
    pub fn parse(value: &str) -> Result<Self, PlaneError> {
        match value {
            "archived" => Ok(Self::Archived),
            "already-archived" => Ok(Self::AlreadyArchived),
            "not-found" => Ok(Self::NotFound),
            "failed" => Ok(Self::Failed),
            _ => Err(PlaneError::bad("unknown archive item status")),
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Archived => "archived",
            Self::AlreadyArchived => "already-archived",
            Self::NotFound => "not-found",
            Self::Failed => "failed",
        }
    }
}

impl std::str::FromStr for ArchiveStatus {
    type Err = PlaneError;
    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Self::parse(value)
    }
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct ArchiveItem {
    pub session_id: String,
    pub status: ArchiveStatus,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct ArchiveReceipt {
    pub operation_id: String,
    pub epoch: String,
    pub deleted_ids: Vec<String>,
    pub results: Vec<ArchiveItem>,
}

pub(super) struct ArchiveRecord {
    pub ids: Vec<String>,
    pub receipt: ArchiveReceipt,
}

pub(super) struct ArchiveOperations {
    pub epoch: String,
    // shortcut: retain receipts for the instance lifetime; move storage if measured memory exceeds the lane budget.
    pub records: Mutex<HashMap<String, ArchiveRecord>>,
}

impl ArchiveOperations {
    pub fn new() -> Self {
        Self {
            epoch: new_id(),
            records: Mutex::new(HashMap::new()),
        }
    }
}
