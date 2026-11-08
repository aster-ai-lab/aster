use std::any::Any;

use async_trait::async_trait;

use crate::{
    PreparedBinding, ProviderDescriptor, ProviderSendPlan, SecretReference, SecretValue, SmsError,
    SubmissionOutcome, SubmitContext,
};

/// Adapter-owned prepared data, consumed once at the remote-effect boundary.
pub trait PreparedSms: Send + Sync {
    fn binding(&self) -> &PreparedBinding;
    fn into_any(self: Box<Self>) -> Box<dyn Any + Send + Sync>;
}

#[async_trait]
pub trait SmsProvider: Send + Sync {
    fn descriptor(&self) -> &ProviderDescriptor;
    /// Validate and construct a request without sending a remote write.
    async fn prepare(&self, plan: ProviderSendPlan) -> Result<Box<dyn PreparedSms>, SmsError>;
    /// Perform at most one HTTP submission. Uncertain failures stay uncertain.
    async fn submit(
        &self,
        prepared: Box<dyn PreparedSms>,
        context: SubmitContext,
    ) -> SubmissionOutcome;
}

/// Hosts resolve explicit, versioned references; providers never inspect ambient keys.
#[async_trait]
pub trait SmsCredentialResolver: Send + Sync {
    async fn resolve(&self, reference: &SecretReference) -> Result<SecretValue, SmsError>;
}
