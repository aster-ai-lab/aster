//! Provider-neutral SMS ports. Adapters are enabled and instantiated explicitly.
//!
//! This library does not provide tenant authorization, durable delivery or billing.
//! A provider acceptance is evidence of submission, never proof of handset delivery.

mod error;
mod provider;
mod types;
mod virtual_provider;
pub use virtual_provider::{VirtualScenario, VirtualSmsProvider};

pub use error::{SmsError, SmsReason};
pub use provider::{PreparedSms, SmsCredentialResolver, SmsProvider};
pub use types::{
    PhoneNumber, PreparedBinding, ProviderDescriptor, ProviderSendPlan, SecretReference,
    SecretValue, SmsCapability, SmsContent, SmsContentKind, SubmissionOutcome,
    SubmissionOutcomeKind, SubmitContext,
};

#[cfg(feature = "smsbao")]
mod providers;
#[cfg(feature = "smsbao")]
pub use providers::smsbao::{
    SmsbaoAuthentication, SmsbaoBalance, SmsbaoConfig, SmsbaoProvider, SmsbaoResponseFormat,
};

#[cfg(test)]
#[path = "types_tests.rs"]
mod tests;
