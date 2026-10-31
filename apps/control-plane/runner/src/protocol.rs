//! The runner side of the pipelines runner protocol
//! (`/v1/api/admin/pipelines/…`).

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

/// Every API answer: `{success, code, message, data}`.
#[derive(Deserialize)]
pub struct Envelope<T> {
    pub data: T,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Heartbeat {
    pub runner_id: String,
    /// Present once, when a registration token is exchanged.
    #[serde(default)]
    pub credential: Option<String>,
}

/// No `Debug`: it carries the job token and the job's secrets.
#[derive(Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ClaimedJob {
    pub job_id: String,
    pub run_id: String,
    pub name: String,
    #[serde(default)]
    pub git_ref: String,
    #[serde(default)]
    pub sha: String,
    #[serde(default)]
    pub steps: Vec<ClaimedStep>,
    #[serde(default)]
    pub repository: Option<String>,
    #[serde(default)]
    pub clone_path: Option<String>,
    #[serde(default)]
    pub job_token: Option<String>,
    #[serde(default)]
    pub job_token_expires_at: Option<String>,
    #[serde(default)]
    pub secrets: BTreeMap<String, String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct ClaimedStep {
    pub name: String,
    pub kind: String,
    #[serde(default)]
    pub run: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum JobStatus {
    Success,
    Failure,
    Cancelled,
}

impl JobStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Success => "success",
            Self::Failure => "failure",
            Self::Cancelled => "cancelled",
        }
    }
}

#[derive(Serialize)]
pub struct TokenBody<'a> {
    pub token: &'a str,
}

#[derive(Serialize)]
pub struct LogBody<'a> {
    pub token: &'a str,
    pub chunk: &'a str,
}

#[derive(Serialize)]
pub struct CompletionBody<'a> {
    pub token: &'a str,
    pub status: &'a str,
}
