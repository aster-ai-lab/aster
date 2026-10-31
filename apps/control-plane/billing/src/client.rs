//! HTTP client for a remote `aster-billing` process.

use std::collections::BTreeMap;

use async_trait::async_trait;
use serde::Deserialize;
use serde_json::{json, Value};

use crate::contexts::billing::domain::{
    BillingCode, IngestCommand, IngestResult, Invoice, InvoiceLine, LineKind,
};

/// Showback metric for member inference. The usage page has no price.
pub const MEMBER_USAGE_METRIC: &str = "total_tokens";
pub const MEMBER_USAGE_FIELD: &str = "tokens";
pub const MEMBER_USAGE_PLAN: &str = "member-usage";
/// 2020-01-01T00:00:00Z. A later ensure must not move this start.
pub const MEMBER_USAGE_STARTED_AT: i64 = 1_577_836_800;

/// One successful `POST /v1/search`.
pub const AGENT_SEARCH_METRIC: &str = "search_request";
pub const AGENT_SEARCH_FIELD: &str = "requests";
/// Pages returned by one successful `POST /v1/parse`.
pub const AGENT_PARSE_METRIC: &str = "parsed_page";
pub const AGENT_PARSE_FIELD: &str = "pages";
pub const AGENT_SERVICES_PLAN: &str = "agent-services";
/// Minor currency units. One search request and one parsed page each cost 1.
pub const AGENT_SEARCH_UNIT_AMOUNT: i64 = 1;
pub const AGENT_PARSE_UNIT_AMOUNT: i64 = 1;

pub fn agent_services_subscription_id(org_id: &str) -> String {
    format!("agent:{org_id}")
}

/// Version 1 keeps the original plan code. Later versions are new rows.
pub fn agent_plan_code(version: u32) -> String {
    if version <= 1 {
        AGENT_SERVICES_PLAN.to_owned()
    } else {
        format!("{AGENT_SERVICES_PLAN}-v{version}")
    }
}

pub fn member_usage_subscription_id(org_id: &str) -> String {
    format!("org:{org_id}")
}

/// Outbound billing port. Production uses [`HttpBillingClient`].
#[async_trait]
pub trait BillingPort: Send + Sync {
    async fn ingest(&self, command: IngestCommand) -> Result<IngestResult, BillingCode>;
    async fn bill(
        &self,
        external_subscription_id: &str,
        timestamp: i64,
    ) -> Result<Invoice, BillingCode>;
}

/// Calls a running `aster-billing` process over HTTP.
#[derive(Clone)]
pub struct HttpBillingClient {
    base: String,
    token: String,
    http: reqwest::Client,
}

impl HttpBillingClient {
    pub fn new(base_url: impl Into<String>, token: impl Into<String>) -> Result<Self, String> {
        let base = base_url.into().trim_end_matches('/').to_string();
        let token = token.into();
        if base.is_empty() {
            return Err("billing service URL is empty".into());
        }
        if token.is_empty() {
            return Err("billing internal token is empty".into());
        }
        Ok(Self {
            base,
            token,
            http: reqwest::Client::new(),
        })
    }

    pub fn base_url(&self) -> &str {
        &self.base
    }

    /// Create the member showback subscription when it is missing.
    /// The internal bearer is the same credential as `POST /v1/events`.
    pub async fn ensure_member_subscription(&self, org_id: &str) -> Result<(), BillingCode> {
        let (_status, envelope) = self
            .post_envelope("/v1/subscriptions", json!({ "org_id": org_id }))
            .await?;
        if envelope.success {
            return Ok(());
        }
        Err(Self::parse_code(&envelope.message))
    }

    async fn post_envelope(&self, path: &str, body: Value) -> Result<(u16, Envelope), BillingCode> {
        let url = format!("{}{}", self.base, path);
        let response = self
            .http
            .post(&url)
            .bearer_auth(&self.token)
            .json(&body)
            .send()
            .await
            .map_err(|_| BillingCode::InvalidRequest)?;
        let status = response.status().as_u16();
        let text = response
            .text()
            .await
            .map_err(|_| BillingCode::InvalidRequest)?;
        let envelope: Envelope =
            serde_json::from_str(&text).map_err(|_| BillingCode::InvalidRequest)?;
        Ok((status, envelope))
    }

    fn parse_code(message: &str) -> BillingCode {
        BillingCode::parse(message).unwrap_or(BillingCode::InvalidRequest)
    }

    fn parse_invoice(data: &Value) -> Result<Invoice, BillingCode> {
        let id = data
            .get("id")
            .and_then(|v| v.as_str())
            .ok_or(BillingCode::InvalidRequest)?
            .to_owned();
        let external_subscription_id = data
            .get("external_subscription_id")
            .and_then(|v| v.as_str())
            .ok_or(BillingCode::InvalidRequest)?
            .to_owned();
        let currency = data
            .get("currency")
            .and_then(|v| v.as_str())
            .unwrap_or("USD")
            .to_owned();
        let period_start = data
            .get("period_start")
            .and_then(|v| v.as_i64())
            .unwrap_or(0);
        let period_end = data.get("period_end").and_then(|v| v.as_i64()).unwrap_or(0);
        let lines = data
            .get("lines")
            .and_then(|v| v.as_array())
            .map(|items| {
                items
                    .iter()
                    .filter_map(|line| {
                        Some(InvoiceLine {
                            kind: match line.get("kind").and_then(|v| v.as_str())? {
                                "recurring" => LineKind::Recurring,
                                _ => LineKind::Usage,
                            },
                            code: line.get("code")?.as_str()?.to_owned(),
                            quantity: line.get("quantity")?.as_i64()?,
                            unit_amount: line.get("unit_amount")?.as_i64()?,
                            amount: line.get("amount")?.as_i64()?,
                        })
                    })
                    .collect()
            })
            .unwrap_or_default();
        Ok(Invoice {
            id,
            external_subscription_id,
            currency,
            period_start,
            period_end,
            lines,
        })
    }
}

#[derive(Debug, Deserialize)]
struct Envelope {
    success: bool,
    #[serde(default)]
    message: String,
    #[serde(default)]
    data: Value,
}

#[async_trait]
impl BillingPort for HttpBillingClient {
    async fn ingest(&self, command: IngestCommand) -> Result<IngestResult, BillingCode> {
        let mut properties = BTreeMap::new();
        for (name, quantity) in &command.properties {
            properties.insert(name.clone(), json!(quantity));
        }
        let (_status, envelope) = self
            .post_envelope(
                "/v1/events",
                json!({
                    "transaction_id": command.transaction_id,
                    "external_subscription_id": command.external_subscription_id,
                    "code": command.code,
                    "timestamp": command.timestamp,
                    "properties": properties,
                }),
            )
            .await?;
        if envelope.success {
            let transaction_id = envelope
                .data
                .get("transaction_id")
                .and_then(|v| v.as_str())
                .unwrap_or(&command.transaction_id)
                .to_owned();
            let duplicate = envelope
                .data
                .get("duplicate")
                .and_then(|v| v.as_bool())
                .unwrap_or(false);
            return Ok(IngestResult::Accepted {
                transaction_id,
                duplicate,
            });
        }
        Ok(IngestResult::Denied {
            error: envelope.message,
        })
    }

    async fn bill(
        &self,
        external_subscription_id: &str,
        timestamp: i64,
    ) -> Result<Invoice, BillingCode> {
        let (_status, envelope) = self
            .post_envelope(
                "/v1/invoices",
                json!({
                    "external_subscription_id": external_subscription_id,
                    "timestamp": timestamp,
                }),
            )
            .await?;
        if envelope.success {
            return Self::parse_invoice(&envelope.data);
        }
        Err(Self::parse_code(&envelope.message))
    }
}
