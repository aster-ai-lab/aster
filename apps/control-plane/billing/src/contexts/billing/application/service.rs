//! Billing use cases. Aggregation and totals stay in `rules`.

use std::collections::BTreeMap;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use uuid::Uuid;

use crate::contexts::billing::domain::repository::BillingStore;
use crate::contexts::billing::domain::rules::{invoice_lines, period_containing, usage_through};
use crate::contexts::billing::domain::{
    BillableMetric, BillingCode, Charge, Customer, IngestCommand, IngestResult, Invoice, Plan,
    Subscription, UsageEvent,
};

pub struct BillingService {
    store: Arc<dyn BillingStore>,
    persists: bool,
}

impl BillingService {
    pub fn new(store: Arc<dyn BillingStore>) -> Self {
        Self {
            store,
            persists: false,
        }
    }

    /// Postgres keeps invoices across restarts. Memory and SQLite do not.
    pub fn with_persistence(mut self, persists: bool) -> Self {
        self.persists = persists;
        self
    }

    pub fn persists(&self) -> bool {
        self.persists
    }

    pub async fn create_customer(&self, external_id: &str, name: &str) -> Result<(), BillingCode> {
        require_id(external_id)?;
        self.store
            .put_customer(Customer {
                external_id: external_id.to_owned(),
                name: name.to_owned(),
            })
            .await
            .map_err(|_| BillingCode::StoreUnavailable)
    }

    pub async fn create_metric(&self, code: &str, field_name: &str) -> Result<(), BillingCode> {
        require_id(code)?;
        require_id(field_name)?;
        self.store
            .put_metric(BillableMetric {
                code: code.to_owned(),
                field_name: field_name.to_owned(),
            })
            .await
            .map_err(|_| BillingCode::StoreUnavailable)
    }

    pub async fn create_plan(
        &self,
        code: &str,
        currency: &str,
        recurring_amount: i64,
        charges: Vec<Charge>,
    ) -> Result<(), BillingCode> {
        require_id(code)?;
        require_id(currency)?;
        if recurring_amount < 0 || charges.iter().any(|charge| charge.unit_amount < 0) {
            return Err(BillingCode::InvalidRequest);
        }
        for charge in &charges {
            if self
                .store
                .metric(&charge.metric_code)
                .await
                .map_err(|_| BillingCode::StoreUnavailable)?
                .is_none()
            {
                return Err(BillingCode::UnknownMetric);
            }
        }
        self.store
            .put_plan(Plan {
                code: code.to_owned(),
                currency: currency.to_owned(),
                recurring_amount,
                charges,
            })
            .await
            .map_err(|_| BillingCode::StoreUnavailable)
    }

    /// Idempotent member showback subscription. An existing row keeps `started_at`.
    pub async fn ensure_member_usage(&self, org_id: &str) -> Result<String, BillingCode> {
        let org_id = org_id.trim();
        require_id(org_id)?;
        if Uuid::parse_str(org_id).is_err() {
            return Err(BillingCode::InvalidRequest);
        }
        if self
            .store
            .metric(crate::client::MEMBER_USAGE_METRIC)
            .await
            .map_err(|_| BillingCode::StoreUnavailable)?
            .is_none()
        {
            self.create_metric(
                crate::client::MEMBER_USAGE_METRIC,
                crate::client::MEMBER_USAGE_FIELD,
            )
            .await?;
        }
        if self
            .store
            .plan(crate::client::MEMBER_USAGE_PLAN)
            .await
            .map_err(|_| BillingCode::StoreUnavailable)?
            .is_none()
        {
            self.create_plan(
                crate::client::MEMBER_USAGE_PLAN,
                "USD",
                0,
                vec![Charge {
                    metric_code: crate::client::MEMBER_USAGE_METRIC.to_owned(),
                    unit_amount: 0,
                }],
            )
            .await?;
        }
        if self
            .store
            .customer(org_id)
            .await
            .map_err(|_| BillingCode::StoreUnavailable)?
            .is_none()
        {
            self.create_customer(org_id, org_id).await?;
        }
        let external_id = crate::client::member_usage_subscription_id(org_id);
        if self
            .store
            .subscription(&external_id)
            .await
            .map_err(|_| BillingCode::StoreUnavailable)?
            .is_none()
        {
            self.create_subscription(
                &external_id,
                org_id,
                crate::client::MEMBER_USAGE_PLAN,
                crate::client::MEMBER_USAGE_STARTED_AT,
            )
            .await?;
        }
        Ok(external_id)
    }

    /// Idempotent search and parse subscription. An existing row keeps `started_at`.
    pub async fn ensure_agent_services(&self, org_id: &str) -> Result<String, BillingCode> {
        let org_id = org_id.trim();
        require_id(org_id)?;
        if Uuid::parse_str(org_id).is_err() {
            return Err(BillingCode::InvalidRequest);
        }
        self.ensure_metric(
            crate::client::AGENT_SEARCH_METRIC,
            crate::client::AGENT_SEARCH_FIELD,
        )
        .await?;
        self.ensure_metric(
            crate::client::AGENT_PARSE_METRIC,
            crate::client::AGENT_PARSE_FIELD,
        )
        .await?;
        if self
            .store
            .customer(org_id)
            .await
            .map_err(|_| BillingCode::StoreUnavailable)?
            .is_none()
        {
            self.create_customer(org_id, org_id).await?;
        }
        let external_id = crate::client::agent_services_subscription_id(org_id);
        if let Some(existing) = self
            .store
            .subscription(&external_id)
            .await
            .map_err(|_| BillingCode::StoreUnavailable)?
        {
            if self.agent_plan_is_complete(&existing.plan_code).await? {
                return Ok(external_id);
            }
            let plan_code = self.complete_agent_plan().await?;
            if existing.plan_code != plan_code {
                self.create_subscription(&external_id, org_id, &plan_code, existing.started_at)
                    .await?;
            }
            return Ok(external_id);
        }
        let plan_code = self.complete_agent_plan().await?;
        self.create_subscription(
            &external_id,
            org_id,
            &plan_code,
            crate::client::MEMBER_USAGE_STARTED_AT,
        )
        .await?;
        Ok(external_id)
    }

    async fn agent_plan_is_complete(&self, code: &str) -> Result<bool, BillingCode> {
        let Some(plan) = self
            .store
            .plan(code)
            .await
            .map_err(|_| BillingCode::StoreUnavailable)?
        else {
            return Ok(false);
        };
        Ok([
            crate::client::AGENT_SEARCH_METRIC,
            crate::client::AGENT_PARSE_METRIC,
        ]
        .iter()
        .all(|metric| {
            plan.charges
                .iter()
                .any(|charge| charge.metric_code == *metric)
        }))
    }

    /// A plan that already exists is left unchanged. A missing charge opens the
    /// next plan version. An existing subscription moves to that version and
    /// keeps its original start time.
    async fn complete_agent_plan(&self) -> Result<String, BillingCode> {
        let required = [
            (
                crate::client::AGENT_SEARCH_METRIC,
                crate::client::AGENT_SEARCH_UNIT_AMOUNT,
            ),
            (
                crate::client::AGENT_PARSE_METRIC,
                crate::client::AGENT_PARSE_UNIT_AMOUNT,
            ),
        ];
        let mut previous = None;
        for version in 1..=32 {
            let code = crate::client::agent_plan_code(version);
            match self
                .store
                .plan(&code)
                .await
                .map_err(|_| BillingCode::StoreUnavailable)?
            {
                Some(plan) => {
                    if required.iter().all(|(metric, _)| {
                        plan.charges
                            .iter()
                            .any(|charge| charge.metric_code == *metric)
                    }) {
                        return Ok(code);
                    }
                    previous = Some(plan);
                }
                None => {
                    let charges = required
                        .iter()
                        .map(|(metric, catalog_amount)| Charge {
                            metric_code: (*metric).to_owned(),
                            unit_amount: previous
                                .as_ref()
                                .and_then(|plan| {
                                    plan.charges
                                        .iter()
                                        .find(|charge| charge.metric_code == *metric)
                                        .map(|charge| charge.unit_amount)
                                })
                                .unwrap_or(*catalog_amount),
                        })
                        .collect();
                    self.create_plan(&code, "USD", 0, charges).await?;
                    return Ok(code);
                }
            }
        }
        Err(BillingCode::InvalidRequest)
    }

    async fn ensure_metric(&self, code: &str, field_name: &str) -> Result<(), BillingCode> {
        if self
            .store
            .metric(code)
            .await
            .map_err(|_| BillingCode::StoreUnavailable)?
            .is_none()
        {
            self.create_metric(code, field_name).await?;
        }
        Ok(())
    }

    pub async fn create_subscription(
        &self,
        external_id: &str,
        external_customer_id: &str,
        plan_code: &str,
        started_at: i64,
    ) -> Result<(), BillingCode> {
        require_id(external_id)?;
        if self
            .store
            .customer(external_customer_id)
            .await
            .map_err(|_| BillingCode::StoreUnavailable)?
            .is_none()
        {
            return Err(BillingCode::UnknownCustomer);
        }
        if self
            .store
            .plan(plan_code)
            .await
            .map_err(|_| BillingCode::StoreUnavailable)?
            .is_none()
        {
            return Err(BillingCode::UnknownPlan);
        }
        self.store
            .put_subscription(Subscription {
                external_id: external_id.to_owned(),
                external_customer_id: external_customer_id.to_owned(),
                plan_code: plan_code.to_owned(),
                started_at,
            })
            .await
            .map_err(|_| BillingCode::StoreUnavailable)
    }

    pub async fn ingest(&self, command: IngestCommand) -> Result<IngestResult, BillingCode> {
        if command.transaction_id.trim().is_empty()
            || command.external_subscription_id.trim().is_empty()
            || command.code.trim().is_empty()
        {
            return Ok(IngestResult::Denied {
                error: BillingCode::InvalidRequest.as_str().to_owned(),
            });
        }
        if let Some(existing) = self
            .store
            .event(&command.transaction_id)
            .await
            .map_err(|_| BillingCode::StoreUnavailable)?
        {
            return replay_usage(self, existing, command).await;
        }
        if self
            .store
            .subscription(&command.external_subscription_id)
            .await
            .map_err(|_| BillingCode::StoreUnavailable)?
            .is_none()
        {
            return Ok(IngestResult::Denied {
                error: BillingCode::UnknownSubscription.as_str().to_owned(),
            });
        }
        let Some(metric) = self
            .store
            .metric(&command.code)
            .await
            .map_err(|_| BillingCode::StoreUnavailable)?
        else {
            return Ok(IngestResult::Denied {
                error: BillingCode::UnknownMetric.as_str().to_owned(),
            });
        };
        let Some(quantity) = quantity_of(&metric, &command.properties) else {
            return Ok(IngestResult::Denied {
                error: BillingCode::InvalidRequest.as_str().to_owned(),
            });
        };
        let timestamp = command.timestamp.unwrap_or_else(now_secs);
        let event = UsageEvent {
            transaction_id: command.transaction_id.clone(),
            external_subscription_id: command.external_subscription_id.clone(),
            code: command.code.clone(),
            timestamp,
            quantity,
        };
        let inserted = self
            .store
            .insert_event(event)
            .await
            .map_err(|_| BillingCode::StoreUnavailable)?;
        if inserted {
            return Ok(IngestResult::Accepted {
                transaction_id: command.transaction_id,
                duplicate: false,
            });
        }
        let Some(existing) = self
            .store
            .event(&command.transaction_id)
            .await
            .map_err(|_| BillingCode::StoreUnavailable)?
        else {
            return Err(BillingCode::StoreUnavailable);
        };
        replay_usage(self, existing, command).await
    }

    pub async fn bill(
        &self,
        external_subscription_id: &str,
        timestamp: i64,
    ) -> Result<Invoice, BillingCode> {
        let Some(subscription) = self
            .store
            .subscription(external_subscription_id)
            .await
            .map_err(|_| BillingCode::StoreUnavailable)?
        else {
            return Err(BillingCode::UnknownSubscription);
        };
        let Some(period) = period_containing(subscription.started_at, timestamp) else {
            return Err(BillingCode::InvalidRequest);
        };
        let Some(plan) = self
            .store
            .plan(&subscription.plan_code)
            .await
            .map_err(|_| BillingCode::StoreUnavailable)?
        else {
            return Err(BillingCode::UnknownPlan);
        };
        let mut kept_id = self
            .store
            .invoice_for_period(external_subscription_id, period.start)
            .await
            .map_err(|_| BillingCode::StoreUnavailable)?
            .map(|invoice| invoice.id);
        for _ in 0..4 {
            let events = self
                .store
                .events_for_subscription(external_subscription_id)
                .await
                .map_err(|_| BillingCode::StoreUnavailable)?;
            let mut quantities = BTreeMap::new();
            for charge in &plan.charges {
                quantities.insert(
                    charge.metric_code.clone(),
                    usage_through(&events, &charge.metric_code, period, timestamp),
                );
            }
            let Some(lines) = invoice_lines(&plan, &quantities) else {
                return Err(BillingCode::InvalidRequest);
            };
            let invoice = Invoice {
                id: kept_id
                    .clone()
                    .unwrap_or_else(|| Uuid::new_v4().to_string()),
                external_subscription_id: external_subscription_id.to_owned(),
                currency: plan.currency.clone(),
                period_start: period.start,
                period_end: period.end,
                lines: lines.clone(),
            };
            self.store
                .save_invoice(invoice)
                .await
                .map_err(|_| BillingCode::StoreUnavailable)?;
            let stored = self
                .store
                .invoice_for_period(external_subscription_id, period.start)
                .await
                .map_err(|_| BillingCode::StoreUnavailable)?
                .ok_or(BillingCode::StoreUnavailable)?;
            kept_id = Some(stored.id.clone());
            if stored.lines == lines
                && stored.currency == plan.currency
                && stored.period_end == period.end
            {
                return Ok(stored);
            }
        }
        Err(BillingCode::StoreUnavailable)
    }

    pub async fn invoice_count(&self) -> Result<usize, BillingCode> {
        self.store
            .invoice_count()
            .await
            .map_err(|_| BillingCode::StoreUnavailable)
    }

    /// Readiness probe: one store round-trip. Used by `GET /health`.
    pub async fn ready(&self) -> Result<(), BillingCode> {
        self.store
            .ping()
            .await
            .map_err(|_| BillingCode::StoreUnavailable)
    }
}

#[async_trait::async_trait]
impl crate::client::BillingPort for BillingService {
    async fn ingest(&self, command: IngestCommand) -> Result<IngestResult, BillingCode> {
        BillingService::ingest(self, command).await
    }

    async fn bill(
        &self,
        external_subscription_id: &str,
        timestamp: i64,
    ) -> Result<Invoice, BillingCode> {
        BillingService::bill(self, external_subscription_id, timestamp).await
    }
}

async fn replay_usage(
    service: &BillingService,
    existing: UsageEvent,
    command: IngestCommand,
) -> Result<IngestResult, BillingCode> {
    let metric = service
        .store
        .metric(&command.code)
        .await
        .map_err(|_| BillingCode::StoreUnavailable)?;
    let quantity = metric
        .as_ref()
        .and_then(|metric| quantity_of(metric, &command.properties));
    // The wall clock places the fact in a period. It is not part of the digest.
    let same = existing.external_subscription_id == command.external_subscription_id
        && existing.code == command.code
        && quantity == Some(existing.quantity);
    if same {
        return Ok(IngestResult::Accepted {
            transaction_id: command.transaction_id,
            duplicate: true,
        });
    }
    Err(BillingCode::Conflict)
}

fn require_id(value: &str) -> Result<(), BillingCode> {
    if value.trim().is_empty() {
        Err(BillingCode::InvalidRequest)
    } else {
        Ok(())
    }
}

fn quantity_of(metric: &BillableMetric, properties: &BTreeMap<String, i64>) -> Option<i64> {
    properties
        .get(&metric.field_name)
        .copied()
        .filter(|quantity| *quantity >= 0)
}

fn now_secs() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs() as i64)
        .unwrap_or(0)
}

#[cfg(test)]
#[path = "service_tests.rs"]
mod tests;
