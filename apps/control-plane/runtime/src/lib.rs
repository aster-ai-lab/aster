//! aster-gateway-runtime — Gateway `/v1` data-plane HTTP surface.

pub mod auth;
pub mod entitlement;
pub mod envelope;
pub mod events;
pub mod http;
pub mod input_contract;
pub mod providers;
pub mod routes;
pub mod usage;
pub mod usage_export;
pub mod vault;

use std::sync::Arc;

use tokio::sync::RwLock;

use crate::events::EventRing;
use crate::usage::UsageStore;
use crate::vault::Vault;

/// Shared process state for the data-plane.
#[derive(Clone)]
pub struct AppState {
    pub vault: Arc<RwLock<Vault>>,
    pub usage: Arc<RwLock<UsageStore>>,
    pub events: Arc<RwLock<EventRing>>,
    pub http: reqwest::Client,
    pub usage_export: crate::usage_export::UsageExporter,
}

impl AppState {
    pub fn new(data_dir: std::path::PathBuf) -> Self {
        let usage = UsageStore::open(&data_dir);
        let vault = Vault::open(&data_dir);
        Self {
            vault: Arc::new(RwLock::new(vault)),
            usage: Arc::new(RwLock::new(usage)),
            events: Arc::new(RwLock::new(EventRing::new(256))),
            http: reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(120))
                .user_agent("aster-runtime/0.1")
                .build()
                .expect("reqwest client"),
            usage_export: crate::usage_export::UsageExporter::disabled(),
        }
    }

    /// Attach the event-spine exporter (main wires NATS from the env).
    pub fn with_usage_export(mut self, exporter: crate::usage_export::UsageExporter) -> Self {
        self.usage_export = exporter;
        self
    }
}

pub const SCOPE_ORG: &str = "org_local";
pub const SCOPE_PROJECT: &str = "proj_local";
pub const SCOPE_ENV: &str = "env_local";

pub const SUPPLIER_VAULT_ENV_MISSING_CODE: &str = "provider.credential_missing";
pub const SUPPLIER_VAULT_ENV_MISSING_MESSAGE: &str =
    "Supplier Vault environment variable is missing.";
