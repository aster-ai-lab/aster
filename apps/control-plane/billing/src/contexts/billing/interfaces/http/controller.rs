//! Inbound HTTP adapter. Usage-event and invoice bodies stay structured.

use std::collections::BTreeMap;
use std::sync::Arc;

use aster_boot::{BootError, BootRequest, BootResponse, Result};
use chrono::{SecondsFormat, Utc};
use serde::Deserialize;
use serde_json::{json, Value};
use uuid::Uuid;

use crate::contexts::billing::application::service::BillingService;
use crate::contexts::billing::domain::{
    BillingCode, IngestCommand, IngestResult, Invoice, InvoiceLine, LineKind,
};

use super::InternalGuard;

pub struct BillingController {
    pub billing: Arc<BillingService>,
    pub internal: InternalGuard,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct IngestBody {
    transaction_id: String,
    external_subscription_id: String,
    code: String,
    timestamp: Option<i64>,
    #[serde(default)]
    properties: BTreeMap<String, Value>,
}

impl IngestBody {
    fn into_command(self) -> std::result::Result<IngestCommand, BillingCode> {
        let mut properties = BTreeMap::new();
        for (name, value) in self.properties {
            let Value::Number(number) = value else {
                return Err(BillingCode::InvalidRequest);
            };
            let Some(quantity) = number.as_i64() else {
                return Err(BillingCode::InvalidRequest);
            };
            properties.insert(name, quantity);
        }
        Ok(IngestCommand {
            transaction_id: self.transaction_id,
            external_subscription_id: self.external_subscription_id,
            code: self.code,
            timestamp: self.timestamp,
            properties,
        })
    }
}

fn invoice_json(invoice: &Invoice) -> Value {
    json!({
        "id": invoice.id,
        "external_subscription_id": invoice.external_subscription_id,
        "currency": invoice.currency,
        "period_start": invoice.period_start,
        "period_end": invoice.period_end,
        "lines": invoice.lines.iter().map(line_json).collect::<Vec<_>>(),
    })
}

fn line_json(line: &InvoiceLine) -> Value {
    json!({
        "kind": match line.kind {
            LineKind::Recurring => "recurring",
            LineKind::Usage => "usage",
        },
        "code": line.code,
        "quantity": line.quantity,
        "unit_amount": line.unit_amount,
        "amount": line.amount,
    })
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct InvoiceBody {
    external_subscription_id: String,
    timestamp: i64,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct MemberSubscriptionBody {
    org_id: String,
}

#[derive(Debug, serde::Serialize)]
struct HealthBody {
    status: &'static str,
}

#[aster_boot::controller("/")]
impl BillingController {
    #[aster_boot::get("/health")]
    async fn health(&self) -> Result<HealthBody> {
        self.billing
            .ready()
            .await
            .map_err(|code| BootError::ServiceUnavailable(code.as_str().to_owned()))?;
        if !self.billing.persists() {
            return Err(BootError::ServiceUnavailable(
                "billing store is ephemeral; set ASTER_BILLING_DATABASE_URL".into(),
            ));
        }
        Ok(HealthBody { status: "ok" })
    }

    #[aster_boot::get("/live")]
    async fn live(&self) -> Result<HealthBody> {
        Ok(HealthBody { status: "up" })
    }

    #[aster_boot::post("/v1/events", raw)]
    async fn events(&self, #[request] request: BootRequest) -> Result<BootResponse> {
        if let Some(response) = self.refuse_anonymous(&request) {
            return response;
        }
        let body = request.body().to_vec();
        if body.is_empty() {
            return denied(&request, BillingCode::InvalidRequest);
        }
        let command = match serde_json::from_slice::<IngestBody>(&body) {
            Ok(body) => match body.into_command() {
                Ok(command) => command,
                Err(code) => return denied(&request, code),
            },
            Err(_) => return denied(&request, BillingCode::InvalidRequest),
        };
        match self.billing.ingest(command).await {
            Ok(IngestResult::Accepted {
                transaction_id,
                duplicate,
            }) => accepted(
                &request,
                json!({
                    "transaction_id": transaction_id,
                    "duplicate": duplicate,
                }),
            ),
            Ok(IngestResult::Denied { error }) => denied_message(&request, &error),
            Err(code) => denied(&request, code),
        }
    }

    #[aster_boot::post("/v1/invoices", raw)]
    async fn invoices(&self, #[request] request: BootRequest) -> Result<BootResponse> {
        if let Some(response) = self.refuse_anonymous(&request) {
            return response;
        }
        let body = request.body().to_vec();
        if body.is_empty() {
            return denied(&request, BillingCode::InvalidRequest);
        }
        let command: InvoiceBody = match serde_json::from_slice(&body) {
            Ok(command) => command,
            Err(_) => return denied(&request, BillingCode::InvalidRequest),
        };
        match self
            .billing
            .bill(&command.external_subscription_id, command.timestamp)
            .await
        {
            Ok(invoice) => accepted(&request, invoice_json(&invoice)),
            Err(code) => denied(&request, code),
        }
    }

    /// Member showback subscription. The caller presents the internal bearer.
    #[aster_boot::post("/v1/subscriptions", raw)]
    async fn subscriptions(&self, #[request] request: BootRequest) -> Result<BootResponse> {
        if let Some(response) = self.refuse_anonymous(&request) {
            return response;
        }
        let body = request.body().to_vec();
        if body.is_empty() {
            return denied(&request, BillingCode::InvalidRequest);
        }
        let command: MemberSubscriptionBody = match serde_json::from_slice(&body) {
            Ok(command) => command,
            Err(_) => return denied(&request, BillingCode::InvalidRequest),
        };
        match self.billing.ensure_member_usage(&command.org_id).await {
            Ok(external_subscription_id) => accepted(
                &request,
                json!({
                    "external_subscription_id": external_subscription_id,
                }),
            ),
            Err(code) => denied(&request, code),
        }
    }

    /// Search and parse subscription. The caller presents the internal bearer.
    #[aster_boot::post("/v1/agent-subscriptions", raw)]
    async fn agent_subscriptions(&self, #[request] request: BootRequest) -> Result<BootResponse> {
        if let Some(response) = self.refuse_anonymous(&request) {
            return response;
        }
        let body = request.body().to_vec();
        if body.is_empty() {
            return denied(&request, BillingCode::InvalidRequest);
        }
        let command: MemberSubscriptionBody = match serde_json::from_slice(&body) {
            Ok(command) => command,
            Err(_) => return denied(&request, BillingCode::InvalidRequest),
        };
        match self.billing.ensure_agent_services(&command.org_id).await {
            Ok(external_subscription_id) => accepted(
                &request,
                json!({
                    "external_subscription_id": external_subscription_id,
                }),
            ),
            Err(code) => denied(&request, code),
        }
    }
}

impl BillingController {
    fn refuse_anonymous(&self, request: &BootRequest) -> Option<Result<BootResponse>> {
        if self.internal.authorize(request.header("authorization")) {
            None
        } else {
            Some(denied(request, BillingCode::Unauthorized))
        }
    }
}

fn accepted(request: &BootRequest, data: Value) -> Result<BootResponse> {
    envelope(request, 200, true, 0, "OK", data)
}

fn denied(request: &BootRequest, code: BillingCode) -> Result<BootResponse> {
    denied_message(request, code.as_str())
}

fn denied_message(request: &BootRequest, error: &str) -> Result<BootResponse> {
    let (status, code) = match error {
        "invalid_request" => (400, 40000),
        "unauthorized" => (401, 40100),
        "unknown_subscription" | "unknown_metric" | "unknown_plan" | "unknown_customer" => {
            (404, 40400)
        }
        "conflict" => (409, 40900),
        "store_unavailable" => (503, 50300),
        _ => (422, 42200),
    };
    envelope(request, status, false, code, error, Value::Null)
}

fn envelope(
    request: &BootRequest,
    status: u16,
    success: bool,
    code: i32,
    message: &str,
    data: Value,
) -> Result<BootResponse> {
    let request_id = request
        .header("x-request-id")
        .and_then(|value| Uuid::parse_str(value.trim()).ok())
        .unwrap_or_else(Uuid::new_v4);
    Ok(BootResponse::json_with_status(
        status,
        &json!({
            "success": success,
            "code": code,
            "message": message,
            "data": data,
            "request_id": request_id,
            "timestamp": Utc::now().to_rfc3339_opts(SecondsFormat::Millis, true),
        }),
    )?
    .with_header("x-request-id", request_id.to_string()))
}
