//! HTTP process adapter. Routes are aster-boot controllers.

mod controller;

use std::sync::Arc;

use aster_boot::{AxumAdapter, BootApplication, HttpAdapter};

use super::super::BillingModule;
use crate::contexts::billing::application::service::BillingService;

pub use controller::BillingController;

/// Internal bearer for usage, invoice, and subscription writes.
/// An unset token keeps those routes closed.
#[derive(Clone)]
pub struct InternalGuard {
    token: Option<String>,
}

impl InternalGuard {
    pub fn closed() -> Self {
        Self { token: None }
    }

    pub fn from_secret(secret: impl Into<String>) -> Self {
        let secret = secret.into();
        Self {
            token: (!secret.is_empty()).then_some(secret),
        }
    }

    pub(crate) fn authorize(&self, authorization: Option<&str>) -> bool {
        let Some(expected) = &self.token else {
            return false;
        };
        let Some(presented) = authorization
            .and_then(|value| {
                value
                    .trim()
                    .strip_prefix("Bearer ")
                    .or_else(|| value.trim().strip_prefix("bearer "))
            })
            .map(str::trim)
        else {
            return false;
        };
        if presented.is_empty() {
            return false;
        }
        constant_time_eq(presented.as_bytes(), expected.as_bytes())
    }
}

fn constant_time_eq(left: &[u8], right: &[u8]) -> bool {
    let max_len = left.len().max(right.len());
    let mut difference = left.len() ^ right.len();
    for index in 0..max_len {
        difference |= usize::from(
            left.get(index).copied().unwrap_or_default()
                != right.get(index).copied().unwrap_or_default(),
        );
    }
    difference == 0
}

pub fn application(
    billing: Arc<BillingService>,
    internal: InternalGuard,
) -> aster_boot::Result<BootApplication> {
    BootApplication::builder()
        .import(BillingModule { billing, internal })
        .build()
}

pub fn router(
    billing: Arc<BillingService>,
    internal: InternalGuard,
) -> aster_boot::Result<axum::Router> {
    AxumAdapter::new().build(application(billing, internal)?)
}

pub async fn serve(
    listener: tokio::net::TcpListener,
    billing: Arc<BillingService>,
    internal: InternalGuard,
) -> std::io::Result<()> {
    let router = router(billing, internal).map_err(|err| std::io::Error::other(err.to_string()))?;
    axum::serve(listener, router).await
}

#[cfg(test)]
#[path = "controller_tests.rs"]
mod tests;
