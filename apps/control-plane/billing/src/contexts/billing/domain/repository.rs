//! Billing store port. Infrastructure implements it with aster-orm.

use async_trait::async_trait;

use super::{BillableMetric, Customer, Invoice, Plan, Subscription, UsageEvent};

#[async_trait]
pub trait BillingStore: Send + Sync {
    async fn put_customer(&self, customer: Customer) -> Result<(), String>;
    async fn customer(&self, external_id: &str) -> Result<Option<Customer>, String>;
    async fn put_metric(&self, metric: BillableMetric) -> Result<(), String>;
    async fn metric(&self, code: &str) -> Result<Option<BillableMetric>, String>;
    async fn put_plan(&self, plan: Plan) -> Result<(), String>;
    async fn plan(&self, code: &str) -> Result<Option<Plan>, String>;
    async fn put_subscription(&self, subscription: Subscription) -> Result<(), String>;
    async fn subscription(&self, external_id: &str) -> Result<Option<Subscription>, String>;
    async fn event(&self, transaction_id: &str) -> Result<Option<UsageEvent>, String>;
    async fn insert_event(&self, event: UsageEvent) -> Result<bool, String>;
    async fn events_for_subscription(&self, external_id: &str) -> Result<Vec<UsageEvent>, String>;
    async fn invoice_for_period(
        &self,
        external_subscription_id: &str,
        period_start: i64,
    ) -> Result<Option<Invoice>, String>;
    async fn insert_invoice(&self, invoice: Invoice) -> Result<bool, String>;
    /// Insert the period invoice, or replace its lines while keeping the stored id.
    async fn save_invoice(&self, invoice: Invoice) -> Result<(), String>;
    async fn invoice_count(&self) -> Result<usize, String>;
    /// Readiness probe: one store round-trip.
    async fn ping(&self) -> Result<(), String>;
}
