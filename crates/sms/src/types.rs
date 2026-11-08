use std::{collections::BTreeMap, fmt, time::Instant};

use zeroize::Zeroizing;

use crate::{error::closed_set, SmsError, SmsReason};

closed_set!(SmsCapability, "SMS capability", {
    Text => "text", Template => "template", StatusById => "status_by_id",
    DeliveryReceipt => "delivery_receipt", NativeIdempotency => "native_idempotency"
});
closed_set!(SmsContentKind, "SMS content kind", { Text => "text", Template => "template" });
closed_set!(SubmissionOutcomeKind, "SMS submission outcome", {
    Accepted => "accepted", DefinitelyNotAccepted => "definitely_not_accepted",
    Indeterminate => "indeterminate"
});

#[derive(Clone, PartialEq, Eq)]
pub struct PhoneNumber(String);

impl PhoneNumber {
    pub fn parse(value: &str) -> Result<Self, SmsError> {
        let valid = value.strip_prefix('+').is_some_and(|digits| {
            (2..=15).contains(&digits.len())
                && !digits.starts_with('0')
                && digits.bytes().all(|byte| byte.is_ascii_digit())
        });
        if !valid {
            return Err(SmsError::Refused {
                reason: SmsReason::InvalidRecipient,
            });
        }
        Ok(Self(value.into()))
    }
    pub fn as_str(&self) -> &str {
        &self.0
    }
}
impl fmt::Debug for PhoneNumber {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("PhoneNumber([REDACTED])")
    }
}

/// Version is mandatory; it is not an environment variable or a credential value.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SecretReference(String);
impl SecretReference {
    pub fn parse(value: &str) -> Result<Self, SmsError> {
        let valid = value.rsplit_once('@').is_some_and(|(id, version)| {
            !id.is_empty()
                && id.len() <= 128
                && id
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'_' | b'-'))
                && !version.starts_with('0')
                && version.bytes().all(|b| b.is_ascii_digit())
                && version.parse::<u64>().is_ok_and(|v| v > 0)
        });
        if !valid {
            return Err(SmsError::InvalidConfiguration {
                field: "secret_reference",
            });
        }
        Ok(Self(value.into()))
    }
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Secret buffers zeroize on drop. Debug deliberately omits their content.
pub struct SecretValue(Zeroizing<String>);
impl SecretValue {
    pub fn new(value: String) -> Result<Self, SmsError> {
        let value = Zeroizing::new(value);
        if value.is_empty() || value.len() > 4096 || value.chars().any(char::is_control) {
            return Err(SmsError::Refused {
                reason: SmsReason::AuthenticationFailed,
            });
        }
        Ok(Self(value))
    }
    /// Borrow secret material only when constructing an authenticated provider request.
    pub fn expose_secret(&self) -> &str {
        &self.0
    }
}
impl fmt::Debug for SecretValue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("SecretValue([REDACTED])")
    }
}

pub enum SmsContent {
    Text(Zeroizing<String>),
    Template {
        id: String,
        variables: BTreeMap<String, String>,
    },
}
impl SmsContent {
    pub fn text(value: String) -> Self {
        Self::Text(Zeroizing::new(value))
    }
    pub fn kind(&self) -> SmsContentKind {
        match self {
            Self::Text(_) => SmsContentKind::Text,
            Self::Template { .. } => SmsContentKind::Template,
        }
    }
}
impl fmt::Debug for SmsContent {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SmsContent")
            .field("kind", &self.kind())
            .finish_non_exhaustive()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PreparedBinding {
    pub profile_id: String,
    pub profile_revision: u64,
    pub attempt_id: String,
}
impl PreparedBinding {
    pub fn validate(&self) -> Result<(), SmsError> {
        if self.profile_id.is_empty()
            || self.profile_id.len() > 128
            || self.profile_revision == 0
            || self.attempt_id.is_empty()
            || self.attempt_id.len() > 128
            || [&self.profile_id, &self.attempt_id]
                .iter()
                .any(|v| v.chars().any(char::is_control))
        {
            return Err(SmsError::Refused {
                reason: SmsReason::InvalidPreparedBinding,
            });
        }
        Ok(())
    }
}

#[derive(Debug)]
pub struct ProviderSendPlan {
    pub recipient: PhoneNumber,
    pub content: SmsContent,
    pub binding: PreparedBinding,
}

#[derive(Debug)]
pub struct ProviderDescriptor {
    pub id: &'static str,
    pub capabilities: &'static [SmsCapability],
    pub allowed_countries: &'static [&'static str],
}

#[derive(Debug, Clone, Copy)]
pub struct SubmitContext {
    pub deadline: Instant,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SubmissionOutcome {
    Accepted {
        provider_message_id: Option<String>,
    },
    DefinitelyNotAccepted {
        reason: SmsReason,
        provider_code: Option<i64>,
    },
    Indeterminate {
        reason: SmsReason,
    },
}
impl SubmissionOutcome {
    pub fn kind(&self) -> SubmissionOutcomeKind {
        match self {
            Self::Accepted { .. } => SubmissionOutcomeKind::Accepted,
            Self::DefinitelyNotAccepted { .. } => SubmissionOutcomeKind::DefinitelyNotAccepted,
            Self::Indeterminate { .. } => SubmissionOutcomeKind::Indeterminate,
        }
    }
}
