mod config;
mod response;

use std::{any::Any, fmt, sync::Arc, time::Instant};

use async_trait::async_trait;
use md5::{Digest, Md5};
use reqwest::Client;
use zeroize::Zeroizing;

use crate::{
    PreparedBinding, PreparedSms, ProviderDescriptor, ProviderSendPlan, SecretValue, SmsCapability,
    SmsContent, SmsCredentialResolver, SmsError, SmsProvider, SmsReason, SubmissionOutcome,
    SubmitContext,
};
pub use config::{SmsbaoAuthentication, SmsbaoConfig, SmsbaoResponseFormat};

const MAX_RESPONSE_BYTES: usize = 4096;
const SEND_URL: &str = "https://api.smsbao.com/sms";
const BALANCE_URL: &str = "https://www.smsbao.com/query";
static DESCRIPTOR: ProviderDescriptor = ProviderDescriptor {
    id: "smsbao",
    capabilities: &[SmsCapability::Text],
    allowed_countries: &["CN"],
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SmsbaoBalance {
    pub remaining: u64,
    pub sent: Option<u64>,
}

pub struct SmsbaoProvider {
    config: SmsbaoConfig,
    client: Client,
    username: SecretValue,
    credential: SecretValue,
    instance: Arc<()>,
    // Fixed in production. Only sibling tests can replace these with loopback fixtures.
    send_url: String,
    balance_url: String,
}
impl fmt::Debug for SmsbaoProvider {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SmsbaoProvider")
            .field("profile_id", &self.config.profile_id)
            .field("profile_revision", &self.config.profile_revision)
            .finish_non_exhaustive()
    }
}

impl SmsbaoProvider {
    pub async fn from_config(
        config: SmsbaoConfig,
        resolver: &dyn SmsCredentialResolver,
    ) -> Result<Self, SmsError> {
        let username = resolver
            .resolve(&config.username_reference)
            .await
            .map_err(|_| auth_error())?;
        let credential = resolver
            .resolve(&config.credential_reference)
            .await
            .map_err(|_| auth_error())?;
        if username.expose_secret().chars().any(char::is_whitespace) {
            return Err(auth_error());
        }
        let credential = match config.authentication {
            SmsbaoAuthentication::Password => SecretValue::new(format!(
                "{:x}",
                Md5::digest(credential.expose_secret().as_bytes())
            ))?,
            SmsbaoAuthentication::PasswordMd5 => {
                if credential.expose_secret().len() != 32
                    || !credential
                        .expose_secret()
                        .bytes()
                        .all(|b| b.is_ascii_hexdigit())
                {
                    return Err(auth_error());
                }
                SecretValue::new(credential.expose_secret().to_ascii_lowercase())?
            }
            SmsbaoAuthentication::ApiKey => credential,
        };
        let client = build_client(config.timeout, true)?;
        Ok(Self {
            config,
            client,
            username,
            credential,
            instance: Arc::new(()),
            send_url: SEND_URL.into(),
            balance_url: BALANCE_URL.into(),
        })
    }

    /// Read-only adapter diagnostic. This is not a delivery-status query.
    pub async fn query_balance(&self) -> Result<SmsbaoBalance, SmsError> {
        let fields = [
            ("u", self.username.expose_secret()),
            ("p", self.credential.expose_secret()),
            ("f", self.config.format.as_str()),
        ];
        let body = self
            .get(
                &self.balance_url,
                &fields,
                Instant::now() + self.config.timeout,
            )
            .await?;
        response::decode_balance(self.config.format, &body)
    }

    async fn get(
        &self,
        endpoint: &str,
        fields: &[(&str, &str)],
        deadline: Instant,
    ) -> Result<Vec<u8>, SmsError> {
        let action = async {
            let mut response = self
                .client
                .get(endpoint)
                .query(fields)
                .send()
                .await
                .map_err(|_| transport_error())?;
            if !response.status().is_success() {
                return Err(transport_error());
            }
            if response
                .content_length()
                .is_some_and(|length| length > MAX_RESPONSE_BYTES as u64)
            {
                return Err(response_error());
            }
            let mut body = Vec::new();
            while let Some(chunk) = response.chunk().await.map_err(|_| transport_error())? {
                if chunk.len() > MAX_RESPONSE_BYTES.saturating_sub(body.len()) {
                    return Err(response_error());
                }
                body.extend_from_slice(&chunk);
            }
            Ok(body)
        };
        tokio::time::timeout_at(deadline.into(), action)
            .await
            .map_err(|_| transport_error())?
    }
}

struct SmsbaoPrepared {
    binding: PreparedBinding,
    instance: Arc<()>,
    phone: Zeroizing<String>,
    content: Zeroizing<String>,
}
impl PreparedSms for SmsbaoPrepared {
    fn binding(&self) -> &PreparedBinding {
        &self.binding
    }
    fn into_any(self: Box<Self>) -> Box<dyn Any + Send + Sync> {
        self
    }
}

#[async_trait]
impl SmsProvider for SmsbaoProvider {
    fn descriptor(&self) -> &ProviderDescriptor {
        &DESCRIPTOR
    }

    async fn prepare(&self, plan: ProviderSendPlan) -> Result<Box<dyn PreparedSms>, SmsError> {
        plan.binding.validate()?;
        if plan.binding.profile_id != self.config.profile_id
            || plan.binding.profile_revision != self.config.profile_revision
        {
            return Err(SmsError::Refused {
                reason: SmsReason::InvalidPreparedBinding,
            });
        }
        let phone = plan
            .recipient
            .as_str()
            .strip_prefix("+86")
            .ok_or(SmsError::Refused {
                reason: SmsReason::InvalidRecipient,
            })?;
        if phone.len() != 11
            || !phone.starts_with('1')
            || !phone
                .as_bytes()
                .get(1)
                .is_some_and(|b| (b'3'..=b'9').contains(b))
        {
            return Err(SmsError::Refused {
                reason: SmsReason::InvalidRecipient,
            });
        }
        let SmsContent::Text(text) = plan.content else {
            return Err(SmsError::Refused {
                reason: SmsReason::UnsupportedCapability,
            });
        };
        if text.trim().is_empty() || text.chars().any(|c| c.is_control() && c != '\n') {
            return Err(SmsError::Refused {
                reason: SmsReason::InvalidContent,
            });
        }
        let prefix_chars = self.config.signature.chars().count() + 2;
        let text_limit = self.config.max_message_chars.saturating_sub(prefix_chars);
        if prefix_chars > self.config.max_message_chars
            || text.chars().take(text_limit + 1).count() > text_limit
        {
            return Err(SmsError::Refused {
                reason: SmsReason::InvalidContent,
            });
        }
        let content = Zeroizing::new(format!("【{}】{}", self.config.signature, text.as_str()));
        Ok(Box::new(SmsbaoPrepared {
            binding: plan.binding,
            instance: Arc::clone(&self.instance),
            phone: Zeroizing::new(phone.into()),
            content,
        }))
    }

    async fn submit(
        &self,
        prepared: Box<dyn PreparedSms>,
        context: SubmitContext,
    ) -> SubmissionOutcome {
        let Ok(prepared) = prepared.into_any().downcast::<SmsbaoPrepared>() else {
            return refused(SmsReason::InvalidPreparedBinding);
        };
        if !Arc::ptr_eq(&prepared.instance, &self.instance) {
            return refused(SmsReason::InvalidPreparedBinding);
        }
        let now = Instant::now();
        if context.deadline <= now {
            return refused(SmsReason::Expired);
        }
        let deadline = context.deadline.min(now + self.config.timeout);
        let fields = [
            ("u", self.username.expose_secret()),
            ("p", self.credential.expose_secret()),
            ("m", prepared.phone.as_str()),
            ("c", prepared.content.as_str()),
            ("f", self.config.format.as_str()),
        ];
        match self.get(&self.send_url, &fields, deadline).await {
            Ok(body) => response::decode_submit(self.config.format, &body),
            Err(SmsError::Refused { reason }) => SubmissionOutcome::Indeterminate { reason },
            Err(_) => SubmissionOutcome::Indeterminate {
                reason: SmsReason::TransportFailure,
            },
        }
    }
}

fn build_client(timeout: std::time::Duration, https_only: bool) -> Result<Client, SmsError> {
    Client::builder()
        .https_only(https_only)
        .no_proxy()
        .timeout(timeout)
        .redirect(reqwest::redirect::Policy::none())
        .retry(reqwest::retry::never())
        .build()
        .map_err(|_| config::invalid("http_client"))
}
fn refused(reason: SmsReason) -> SubmissionOutcome {
    SubmissionOutcome::DefinitelyNotAccepted {
        reason,
        provider_code: None,
    }
}
fn auth_error() -> SmsError {
    SmsError::Refused {
        reason: SmsReason::AuthenticationFailed,
    }
}
fn transport_error() -> SmsError {
    SmsError::Refused {
        reason: SmsReason::TransportFailure,
    }
}
fn response_error() -> SmsError {
    SmsError::Refused {
        reason: SmsReason::UnrecognizedProviderResponse,
    }
}

#[cfg(test)]
#[path = "tests.rs"]
mod tests;
