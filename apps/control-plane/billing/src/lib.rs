//! Usage-to-invoice billing for Aster OS.
//!
//! This process owns customers, plans, subscriptions, and invoices.
//! It does not grant resources, mint capability keys, or open directory sessions.
//!
//! Feature `client`: domain DTOs + [`HttpBillingClient`].
//! Feature `server`: ORM store + Boot HTTP surface (binary).

mod client;
mod contexts;

#[cfg(test)]
mod architecture;

#[cfg(feature = "server")]
use std::sync::Arc;

pub use client::{
    member_usage_subscription_id, BillingPort, HttpBillingClient, MEMBER_USAGE_FIELD,
    MEMBER_USAGE_METRIC, MEMBER_USAGE_PLAN, MEMBER_USAGE_STARTED_AT,
};
pub use contexts::billing::domain::repository::BillingStore;
pub use contexts::billing::domain::rules::usage_quantity;
pub use contexts::billing::domain::{
    BillableMetric, BillingCode, Charge, Customer, IngestCommand, IngestResult, Invoice,
    InvoiceLine, LineKind, Plan, Subscription, UsageEvent,
};

#[cfg(feature = "server")]
pub use contexts::billing::application::service::BillingService;
#[cfg(feature = "server")]
pub use contexts::billing::infrastructure::sql::SqlStore;
#[cfg(feature = "server")]
pub use contexts::billing::interfaces::http::{router, serve, InternalGuard};

/// In-memory store for tests. The `aster-billing` process does not call this.
#[cfg(feature = "server")]
pub async fn open_memory() -> Result<BillingService, String> {
    let store = SqlStore::open_in_memory().await?;
    Ok(BillingService::new(Arc::new(store)))
}

#[cfg(feature = "server")]
pub async fn open_sqlite(path: impl AsRef<std::path::Path>) -> Result<BillingService, String> {
    let store = SqlStore::open_sqlite(path).await?;
    Ok(BillingService::new(Arc::new(store)))
}

#[cfg(feature = "server")]
pub async fn open_postgres(database_url: &str) -> Result<BillingService, String> {
    let store = SqlStore::open_postgres(database_url).await?;
    Ok(BillingService::new(Arc::new(store)).with_persistence(true))
}
