//! Billing domain: customers, plans, usage, and invoices.
//!
//! No HTTP framework and no SQL driver in this module.

use std::collections::BTreeMap;

pub mod repository;
pub mod rules;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BillingCode {
    InvalidRequest,
    Unauthorized,
    UnknownSubscription,
    UnknownMetric,
    UnknownPlan,
    UnknownCustomer,
    /// The same transaction id arrived with a different usage fact.
    Conflict,
    /// The store could not complete the read or write.
    StoreUnavailable,
}

impl BillingCode {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::InvalidRequest => "invalid_request",
            Self::Unauthorized => "unauthorized",
            Self::UnknownSubscription => "unknown_subscription",
            Self::UnknownMetric => "unknown_metric",
            Self::UnknownPlan => "unknown_plan",
            Self::UnknownCustomer => "unknown_customer",
            Self::Conflict => "conflict",
            Self::StoreUnavailable => "store_unavailable",
        }
    }

    /// Refuses an unknown value and names every allowed code.
    pub fn parse(raw: &str) -> Result<Self, String> {
        match raw {
            "invalid_request" => Ok(Self::InvalidRequest),
            "unauthorized" => Ok(Self::Unauthorized),
            "unknown_subscription" => Ok(Self::UnknownSubscription),
            "unknown_metric" => Ok(Self::UnknownMetric),
            "unknown_plan" => Ok(Self::UnknownPlan),
            "unknown_customer" => Ok(Self::UnknownCustomer),
            "conflict" => Ok(Self::Conflict),
            "store_unavailable" => Ok(Self::StoreUnavailable),
            other => Err(format!(
                "billing code must be one of invalid_request, unauthorized, unknown_subscription, unknown_metric, unknown_plan, unknown_customer, conflict, store_unavailable, got {other:?}"
            )),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Customer {
    pub external_id: String,
    pub name: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BillableMetric {
    pub code: String,
    pub field_name: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Charge {
    pub metric_code: String,
    pub unit_amount: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Plan {
    pub code: String,
    pub currency: String,
    pub recurring_amount: i64,
    pub charges: Vec<Charge>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Subscription {
    pub external_id: String,
    pub external_customer_id: String,
    pub plan_code: String,
    pub started_at: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UsageEvent {
    pub transaction_id: String,
    pub external_subscription_id: String,
    pub code: String,
    pub timestamp: i64,
    pub quantity: i64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LineKind {
    Recurring,
    Usage,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InvoiceLine {
    pub kind: LineKind,
    pub code: String,
    pub quantity: i64,
    pub unit_amount: i64,
    pub amount: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Invoice {
    pub id: String,
    pub external_subscription_id: String,
    pub currency: String,
    pub period_start: i64,
    pub period_end: i64,
    pub lines: Vec<InvoiceLine>,
}

impl Invoice {
    pub fn total(&self) -> i64 {
        self.lines.iter().map(|line| line.amount).sum()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IngestResult {
    Accepted {
        transaction_id: String,
        duplicate: bool,
    },
    Denied {
        error: String,
    },
}

#[derive(Debug, Clone)]
pub struct IngestCommand {
    pub transaction_id: String,
    pub external_subscription_id: String,
    pub code: String,
    pub timestamp: Option<i64>,
    pub properties: BTreeMap<String, i64>,
}
