//! Deterministic, zero-network provider. Hosting applications gate sandbox use.
use crate::*;
use async_trait::async_trait;
use std::{
    any::Any,
    sync::{Arc, Mutex},
    time::Instant,
};
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VirtualScenario {
    Accept,
    Reject,
    Unknown,
}
impl VirtualScenario {
    pub fn parse(value: &str) -> Result<Self, SmsError> {
        match value {
            "accept" => Ok(Self::Accept),
            "reject" => Ok(Self::Reject),
            "unknown" => Ok(Self::Unknown),
            _ => Err(SmsError::InvalidClosedSet {
                set: "virtual SMS scenario",
            }),
        }
    }
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Accept => "accept",
            Self::Reject => "reject",
            Self::Unknown => "unknown",
        }
    }
}
pub struct VirtualSmsProvider {
    profile: String,
    revision: u64,
    scenario: VirtualScenario,
    instance: Arc<()>,
    inbox: Mutex<Vec<ProviderSendPlan>>,
}
impl VirtualSmsProvider {
    pub fn new(
        profile: String,
        revision: u64,
        scenario: VirtualScenario,
    ) -> Result<Self, SmsError> {
        PreparedBinding {
            profile_id: profile.clone(),
            profile_revision: revision,
            attempt_id: "validation".into(),
        }
        .validate()?;
        Ok(Self {
            profile,
            revision,
            scenario,
            instance: Arc::new(()),
            inbox: Mutex::new(Vec::new()),
        })
    }
    /// Local tests own this inbox; production hosts must never expose it without authority.
    pub fn accepted_count(&self) -> Result<usize, SmsError> {
        self.inbox
            .lock()
            .map(|v| v.len())
            .map_err(|_| SmsError::Refused {
                reason: SmsReason::ProviderUnavailable,
            })
    }
}
struct VirtualPrepared {
    plan: ProviderSendPlan,
    instance: Arc<()>,
}
impl PreparedSms for VirtualPrepared {
    fn binding(&self) -> &PreparedBinding {
        &self.plan.binding
    }
    fn into_any(self: Box<Self>) -> Box<dyn Any + Send + Sync> {
        self
    }
}
static DESCRIPTOR: ProviderDescriptor = ProviderDescriptor {
    id: "virtual",
    capabilities: &[SmsCapability::Text],
    allowed_countries: &[],
};
#[async_trait]
impl SmsProvider for VirtualSmsProvider {
    fn descriptor(&self) -> &ProviderDescriptor {
        &DESCRIPTOR
    }
    async fn prepare(&self, plan: ProviderSendPlan) -> Result<Box<dyn PreparedSms>, SmsError> {
        plan.binding.validate()?;
        if plan.binding.profile_id != self.profile || plan.binding.profile_revision != self.revision
        {
            return Err(SmsError::Refused {
                reason: SmsReason::InvalidPreparedBinding,
            });
        }
        let SmsContent::Text(text) = &plan.content else {
            return Err(SmsError::Refused {
                reason: SmsReason::UnsupportedCapability,
            });
        };
        if text.trim().is_empty() || text.chars().take(4097).count() > 4096 {
            return Err(SmsError::Refused {
                reason: SmsReason::InvalidContent,
            });
        }
        Ok(Box::new(VirtualPrepared {
            plan,
            instance: self.instance.clone(),
        }))
    }
    async fn submit(
        &self,
        prepared: Box<dyn PreparedSms>,
        context: SubmitContext,
    ) -> SubmissionOutcome {
        let Ok(prepared) = prepared.into_any().downcast::<VirtualPrepared>() else {
            return reject(SmsReason::InvalidPreparedBinding);
        };
        if !Arc::ptr_eq(&self.instance, &prepared.instance) {
            return reject(SmsReason::InvalidPreparedBinding);
        }
        if context.deadline <= Instant::now() {
            return reject(SmsReason::Expired);
        }
        match self.scenario {
            VirtualScenario::Reject => reject(SmsReason::ProviderRejected),
            VirtualScenario::Unknown => SubmissionOutcome::Indeterminate {
                reason: SmsReason::TransportFailure,
            },
            VirtualScenario::Accept => {
                let id = format!("virtual:{}", prepared.plan.binding.attempt_id);
                let Ok(mut inbox) = self.inbox.lock() else {
                    return SubmissionOutcome::Indeterminate {
                        reason: SmsReason::ProviderUnavailable,
                    };
                };
                inbox.push(prepared.plan);
                SubmissionOutcome::Accepted {
                    provider_message_id: Some(id),
                }
            }
        }
    }
}
fn reject(reason: SmsReason) -> SubmissionOutcome {
    SubmissionOutcome::DefinitelyNotAccepted {
        reason,
        provider_code: None,
    }
}
#[cfg(test)]
#[path = "virtual_tests.rs"]
mod tests;
