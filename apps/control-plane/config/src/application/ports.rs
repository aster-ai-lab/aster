//! Application ports — outbound adapters for config fan-out / read.
//! Lives in application (not domain) because the payload is an integration event.

use async_trait::async_trait;
use tokio::sync::mpsc;

use crate::error::AppResult;

use super::super::domain::ConfigEntry;
use super::ConfigChangedEvent;

/// Narrow read port for cross-context consumers (model_supply / model_hub).
/// Avoids dragging the full ConfigCenterService surface across BC boundaries.
#[async_trait]
pub trait ConfigReader: Send + Sync {
    async fn get(&self, namespace: &str, key: &str) -> AppResult<ConfigEntry>;
}

#[async_trait]
pub trait ConfigChangeBus: Send + Sync {
    async fn publish(&self, event: &ConfigChangedEvent) -> Result<(), String>;

    /// Background listen: each remote event is forwarded on `tx`.
    fn spawn_listen(&self, tx: mpsc::Sender<ConfigChangedEvent>);
}
