//! Session JSON at the store edge. The domain record itself is not a serde type.

use serde::{Deserialize, Serialize};

use super::super::domain::repository::SessionRecord;
use super::super::domain::{AuthError, Plane};

#[derive(Debug, Serialize, Deserialize)]
struct SessionJson {
    token_hash: String,
    plane: String,
    subject_id: uuid::Uuid,
    org_id: Option<uuid::Uuid>,
    auth_time: i64,
    amr: Vec<String>,
    expires_at: i64,
    #[serde(default)]
    computer_id: Option<String>,
}

impl From<&SessionRecord> for SessionJson {
    fn from(session: &SessionRecord) -> Self {
        Self {
            token_hash: session.token_hash.clone(),
            plane: session.plane.as_str().to_owned(),
            subject_id: session.subject_id,
            org_id: session.org_id,
            auth_time: session.auth_time,
            amr: session.amr.clone(),
            expires_at: session.expires_at,
            computer_id: session.computer_id.clone(),
        }
    }
}

fn record_from_json(json: SessionJson) -> Result<SessionRecord, AuthError> {
    let plane = Plane::parse(&json.plane).ok_or_else(|| AuthError::storage("unknown plane"))?;
    Ok(SessionRecord {
        token_hash: json.token_hash,
        plane,
        subject_id: json.subject_id,
        org_id: json.org_id,
        auth_time: json.auth_time,
        amr: json.amr,
        expires_at: json.expires_at,
        computer_id: json.computer_id,
    })
}

pub fn encode_session(session: &SessionRecord) -> Result<String, AuthError> {
    serde_json::to_string(&SessionJson::from(session))
        .map_err(|err| AuthError::storage(err.to_string()))
}

pub fn encode_session_value(session: &SessionRecord) -> Result<serde_json::Value, AuthError> {
    serde_json::to_value(SessionJson::from(session))
        .map_err(|err| AuthError::storage(err.to_string()))
}

pub fn decode_session(raw: &str) -> Result<SessionRecord, AuthError> {
    let json = serde_json::from_str(raw).map_err(|err| AuthError::storage(err.to_string()))?;
    record_from_json(json)
}

pub fn decode_session_value(value: serde_json::Value) -> Result<SessionRecord, AuthError> {
    let json = serde_json::from_value(value).map_err(|err| AuthError::storage(err.to_string()))?;
    record_from_json(json)
}
