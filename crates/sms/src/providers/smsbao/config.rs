use std::time::Duration;

use aster_acl::{Block, Value};

use crate::{error::closed_set, SecretReference, SmsError};

closed_set!(SmsbaoAuthentication, "SMSBAO authentication", {
    Password => "password", PasswordMd5 => "password_md5", ApiKey => "api_key"
});
closed_set!(SmsbaoResponseFormat, "SMSBAO response format", { Json => "json", Text => "txt" });

#[derive(Debug, Clone)]
pub struct SmsbaoConfig {
    pub(super) profile_id: String,
    pub(super) profile_revision: u64,
    pub(super) signature: String,
    pub(super) username_reference: SecretReference,
    pub(super) credential_reference: SecretReference,
    pub(super) authentication: SmsbaoAuthentication,
    pub(super) format: SmsbaoResponseFormat,
    pub(super) timeout: Duration,
    pub(super) max_message_chars: usize,
}

impl SmsbaoConfig {
    /// Parse only ACL and refuse unknown fields, nested blocks and missing values.
    pub fn from_acl(source: &str) -> Result<Self, SmsError> {
        let document = aster_acl::parse(source).map_err(|_| invalid("acl_document"))?;
        let [block] = document.blocks.as_slice() else {
            return Err(invalid("provider"));
        };
        if block.name != "provider" || block.labels != ["smsbao"] || !block.blocks.is_empty() {
            return Err(invalid("provider"));
        }
        const FIELDS: &[&str] = &[
            "profile_id",
            "profile_revision",
            "signature",
            "username_secret_ref",
            "credential_secret_ref",
            "auth_method",
            "response_format",
            "timeout_ms",
            "max_message_chars",
        ];
        if block
            .attributes
            .keys()
            .any(|key| !FIELDS.contains(&key.as_str()))
        {
            return Err(invalid("unknown_attribute"));
        }
        let profile_id = string(block, "profile_id")?;
        if profile_id.is_empty()
            || profile_id.len() > 128
            || !profile_id
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'_' | b'-'))
        {
            return Err(invalid("profile_id"));
        }
        let signature = string(block, "signature")?;
        if signature.trim() != signature
            || signature.is_empty()
            || signature.chars().count() > 32
            || signature
                .chars()
                .any(|c| c.is_control() || matches!(c, '[' | ']' | '【' | '】' | '〖' | '〗'))
        {
            return Err(invalid("signature"));
        }
        Ok(Self {
            profile_id,
            profile_revision: integer(block, "profile_revision", 1, 1_000_000)?,
            signature,
            username_reference: SecretReference::parse(&string(block, "username_secret_ref")?)?,
            credential_reference: SecretReference::parse(&string(block, "credential_secret_ref")?)?,
            authentication: SmsbaoAuthentication::parse(&string(block, "auth_method")?)?,
            format: SmsbaoResponseFormat::parse(&string(block, "response_format")?)?,
            timeout: Duration::from_millis(integer(block, "timeout_ms", 1, 60_000)?),
            max_message_chars: integer(block, "max_message_chars", 1, 4096)? as usize,
        })
    }
    pub fn credential_references(&self) -> [&crate::SecretReference; 2] {
        [&self.username_reference, &self.credential_reference]
    }
    pub fn profile_id(&self) -> &str {
        &self.profile_id
    }
    pub fn profile_revision(&self) -> u64 {
        self.profile_revision
    }
}

fn string(block: &Block, field: &'static str) -> Result<String, SmsError> {
    match block.attributes.get(field) {
        Some(Value::String(value)) => Ok(value.clone()),
        _ => Err(invalid(field)),
    }
}
fn integer(
    block: &Block,
    field: &'static str,
    minimum: u64,
    maximum: u64,
) -> Result<u64, SmsError> {
    match block.attributes.get(field) {
        Some(Value::Number(value))
            if value.is_finite()
                && value.fract() == 0.0
                && *value >= minimum as f64
                && *value <= maximum as f64 =>
        {
            Ok(*value as u64)
        }
        _ => Err(invalid(field)),
    }
}
pub(super) fn invalid(field: &'static str) -> SmsError {
    SmsError::InvalidConfiguration { field }
}

#[cfg(test)]
#[path = "config_tests.rs"]
mod tests;
