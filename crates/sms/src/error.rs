use thiserror::Error;

macro_rules! closed_set {
    ($name:ident, $set:literal, {$($variant:ident => $text:literal),+ $(,)?}) => {
        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        pub enum $name { $($variant),+ }
        impl $name {
            pub const ALL: &'static [Self] = &[$(Self::$variant),+];
            pub const fn as_str(self) -> &'static str {
                match self { $(Self::$variant => $text),+ }
            }
            pub fn parse(value: &str) -> Result<Self, crate::SmsError> {
                match value {
                    $($text => Ok(Self::$variant)),+,
                    _ => Err(crate::SmsError::InvalidClosedSet { set: $set }),
                }
            }
        }
    };
}
pub(crate) use closed_set;

closed_set!(SmsReason, "SMS reason", {
    InvalidConfiguration => "invalid_configuration",
    InvalidRecipient => "invalid_recipient",
    InvalidContent => "invalid_content",
    UnsupportedCapability => "unsupported_capability",
    AuthenticationFailed => "authentication_failed",
    QuotaExceeded => "quota_exceeded",
    PolicyDenied => "policy_denied",
    ProviderUnavailable => "provider_unavailable",
    ProviderRejected => "provider_rejected",
    Expired => "expired",
    TransportFailure => "transport_failure",
    UnrecognizedProviderResponse => "unrecognized_provider_response",
    InvalidPreparedBinding => "invalid_prepared_binding"
});

/// Errors contain only safe, stable reasons, never a request URL or wire body.
#[derive(Debug, Error)]
pub enum SmsError {
    #[error("invalid closed set: {set}")]
    InvalidClosedSet { set: &'static str },
    #[error("invalid SMS configuration field: {field}")]
    InvalidConfiguration { field: &'static str },
    #[error("SMS operation refused: {reason:?}")]
    Refused { reason: SmsReason },
    #[error("SMS provider refused request: {reason:?} (code {code})")]
    Provider { reason: SmsReason, code: i64 },
}
