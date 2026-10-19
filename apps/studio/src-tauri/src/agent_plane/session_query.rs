use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};

use super::{domain::Session, error::PlaneError};

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub(super) enum ExecutionStatus {
    Pending,
    Streaming,
    AwaitingApproval,
    AwaitingForm,
    Done,
    Error,
    Aborted,
    Unknown,
}

impl ExecutionStatus {
    pub fn parse(value: &str) -> Result<Self, PlaneError> {
        match value {
            "pending" => Ok(Self::Pending),
            "streaming" => Ok(Self::Streaming),
            "awaiting-approval" => Ok(Self::AwaitingApproval),
            "awaiting-form" => Ok(Self::AwaitingForm),
            "done" => Ok(Self::Done),
            "error" => Ok(Self::Error),
            "aborted" => Ok(Self::Aborted),
            "unknown" => Ok(Self::Unknown),
            _ => Err(PlaneError::bad("unknown session execution status")),
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::Streaming => "streaming",
            Self::AwaitingApproval => "awaiting-approval",
            Self::AwaitingForm => "awaiting-form",
            Self::Done => "done",
            Self::Error => "error",
            Self::Aborted => "aborted",
            Self::Unknown => "unknown",
        }
    }

    pub fn is_live(self) -> bool {
        matches!(
            self,
            Self::Pending | Self::Streaming | Self::AwaitingApproval | Self::AwaitingForm
        )
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct SessionExecution {
    pub status: ExecutionStatus,
    pub execution_id: String,
    pub anchor_message_id: String,
    pub observed_at: String,
}

#[derive(Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct SessionQuery {
    pub agent_id: Option<String>,
    pub workspace_id: Option<String>,
    pub ids: Vec<String>,
    pub in_trash: bool,
    pub search: Option<String>,
    pub status: Option<ExecutionStatus>,
    pub activity_from: Option<String>,
    pub activity_before: Option<String>,
}

#[derive(Serialize)]
pub(super) struct ListedSession {
    #[serde(flatten)]
    pub session: Session,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub execution: Option<SessionExecution>,
}

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Boundary {
    version: u8,
    query: String,
    activity: String,
    id: String,
}

fn instant(value: &str) -> Result<DateTime<Utc>, PlaneError> {
    DateTime::parse_from_rfc3339(value)
        .map(|time| time.with_timezone(&Utc))
        .map_err(|_| PlaneError::bad("session activity boundary must be an RFC3339 instant"))
}

impl SessionQuery {
    pub fn normalized(mut self) -> Result<Self, PlaneError> {
        self.search = self
            .search
            .map(|value| value.trim().to_owned())
            .filter(|value| !value.is_empty());
        if self
            .search
            .as_ref()
            .is_some_and(|text| text.chars().count() > 255)
        {
            return Err(PlaneError::bad("session search exceeds 255 characters"));
        }
        self.search = self.search.map(|value| value.to_lowercase());
        if self.ids.len() > 200 || self.ids.iter().any(|id| id.is_empty()) {
            return Err(PlaneError::bad(
                "session ids must contain at most 200 nonempty IDs",
            ));
        }
        if self.workspace_id.as_ref().is_some_and(|id| id.is_empty()) {
            return Err(PlaneError::bad("session workspaceId must not be empty"));
        }
        self.ids.sort();
        self.ids.dedup();
        for boundary in [&mut self.activity_from, &mut self.activity_before] {
            if let Some(value) = boundary {
                *value = instant(value)?.to_rfc3339();
            }
        }
        if let (Some(from), Some(before)) = (&self.activity_from, &self.activity_before) {
            if instant(from)? >= instant(before)? {
                return Err(PlaneError::bad(
                    "session activity interval must be increasing",
                ));
            }
        }
        Ok(self)
    }

    pub fn matches(&self, row: &ListedSession) -> bool {
        let session = &row.session;
        let status = row
            .execution
            .as_ref()
            .map(|entry| entry.status)
            .unwrap_or(ExecutionStatus::Unknown);
        let activity_matches = || -> Result<bool, PlaneError> {
            if self.activity_from.is_none() && self.activity_before.is_none() {
                return Ok(true);
            }
            let time = instant(&session.last_activity_at)?;
            Ok(self
                .activity_from
                .as_ref()
                .map(|value| instant(value))
                .transpose()?
                .is_none_or(|from| time >= from)
                && self
                    .activity_before
                    .as_ref()
                    .map(|value| instant(value))
                    .transpose()?
                    .is_none_or(|before| time < before))
        };
        self.search
            .as_ref()
            .is_none_or(|needle| session.name.to_lowercase().contains(needle))
            && self.status.is_none_or(|expected| expected == status)
            && activity_matches().unwrap_or(false)
    }

    pub fn page(
        &self,
        mut rows: Vec<ListedSession>,
        cursor: Option<&str>,
        limit: usize,
    ) -> Result<Value, PlaneError> {
        if !(1..=200).contains(&limit) {
            return Err(PlaneError::bad("session limit must be between 1 and 200"));
        }
        let bytes =
            serde_json::to_vec(self).map_err(|error| PlaneError::Internal(error.to_string()))?;
        let fingerprint = format!("{:x}", Sha256::digest(bytes));
        let total = rows.len();
        if let Some(cursor) = cursor {
            let legacy = self.search.is_none()
                && self.status.is_none()
                && self.activity_from.is_none()
                && self.activity_before.is_none();
            if legacy && let Ok(offset) = cursor.parse::<usize>() {
                rows.drain(..offset.min(rows.len()));
            } else {
                if cursor.len() > 4096 {
                    return Err(PlaneError::bad("session cursor is invalid"));
                }
                let boundary = URL_SAFE_NO_PAD
                    .decode(cursor)
                    .ok()
                    .and_then(|bytes| serde_json::from_slice::<Boundary>(&bytes).ok())
                    .filter(|boundary| {
                        boundary.version == 1
                            && boundary.query == fingerprint
                            && !boundary.id.is_empty()
                            && boundary.id.chars().count() <= 200
                            && instant(&boundary.activity).is_ok()
                    })
                    .ok_or_else(|| PlaneError::bad("session cursor is invalid for this query"))?;
                rows.retain(|row| {
                    row.session.last_activity_at < boundary.activity
                        || (row.session.last_activity_at == boundary.activity
                            && row.session.id > boundary.id)
                });
            }
        }
        let has_more = rows.len() > limit;
        rows.truncate(limit);
        let next = if has_more {
            rows.last()
                .map(|row| Boundary {
                    version: 1,
                    query: fingerprint,
                    activity: row.session.last_activity_at.clone(),
                    id: row.session.id.clone(),
                })
                .map(|boundary| {
                    serde_json::to_vec(&boundary)
                        .map(|bytes| URL_SAFE_NO_PAD.encode(bytes))
                        .map_err(|error| PlaneError::Internal(error.to_string()))
                })
                .transpose()?
        } else {
            None
        };
        let mut result = json!({ "items": rows, "total": total });
        if let Some(next) = next {
            result["nextCursor"] = json!(next);
        }
        Ok(result)
    }
}
