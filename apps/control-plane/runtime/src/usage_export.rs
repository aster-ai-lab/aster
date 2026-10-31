//! Best-effort usage event publishing onto the event spine
//! (`events.runtime.*`) — the producer half of the metering loop.
//!
//! Per-record publishing is fire-and-forget: a spine outage logs at error
//! level and never blocks a request. The batch endpoint awaits its publish
//! and reports the real outcome. With no NATS origin configured the
//! exporter refuses closed and the export endpoint keeps its named 501.

use aster_event::provider::nats::{NatsClient, NatsConfig};
use aster_event::types::Event;
use async_trait::async_trait;

use crate::usage::UsageRecord;

/// The export schema advertised on every batch event.
pub const USAGE_BATCH_SCHEMA: &str = "aster.gateway.usage-batch.v1";
/// The subject the queue workers project (`usage-ingest`).
pub const USAGE_EVENT_SUBJECT: &str = "events.runtime.usage";
/// Environment variable enabling the spine (NATS URL). Unset refuses closed.
pub const NATS_URL_ENV: &str = "ASTER_RUNTIME_NATS_URL";

/// One metering batch as it crosses the spine.
#[derive(Debug, Clone)]
pub struct UsageBatch {
    pub gateway_id: String,
    pub records: Vec<UsageRecord>,
}

/// Outbound metering transport. Production publishes through aster-event's
/// NATS provider; tests inject an in-memory double.
#[async_trait]
pub trait UsageExport: Send + Sync {
    async fn publish(&self, batch: UsageBatch) -> Result<(), String>;
}

/// NATS (JetStream) exporter over the platform event abstraction.
pub struct NatsUsageExport {
    client: NatsClient,
}

impl NatsUsageExport {
    /// Connect from `ASTER_RUNTIME_NATS_URL`. `None` when unset or empty
    /// (refused closed); `Some(Err)` names the connection failure.
    pub async fn connect_from_env() -> Option<Result<Self, String>> {
        let url = std::env::var(NATS_URL_ENV).ok()?;
        if url.trim().is_empty() {
            return None;
        }
        let config = NatsConfig {
            url,
            stream_name: "ASTER_RUNTIME_USAGE".to_string(),
            subject_prefix: "events.runtime".to_string(),
            ..NatsConfig::default()
        };
        match NatsClient::connect(config).await {
            Ok(client) => Some(Ok(Self { client })),
            Err(error) => Some(Err(error.to_string())),
        }
    }
}

#[async_trait]
impl UsageExport for NatsUsageExport {
    async fn publish(&self, batch: UsageBatch) -> Result<(), String> {
        let event = batch_event(&batch);
        self.client
            .publish(&event)
            .await
            .map(|_| ())
            .map_err(|error| error.to_string())
    }
}

/// Build the spine event for one batch.
pub fn batch_event(batch: &UsageBatch) -> Event {
    let records: Vec<serde_json::Value> = batch
        .records
        .iter()
        .map(|record| serde_json::to_value(record).unwrap_or(serde_json::Value::Null))
        .collect();
    Event::typed(
        USAGE_EVENT_SUBJECT,
        "runtime",
        USAGE_BATCH_SCHEMA,
        1,
        format!("usage batch of {} record(s)", records.len()),
        batch.gateway_id.clone(),
        serde_json::json!({
            "schema": USAGE_BATCH_SCHEMA,
            "gateway_id": batch.gateway_id,
            "records": records,
            "count": records.len(),
        }),
    )
}

/// Publisher handle shared through the request state.
#[derive(Clone, Default)]
pub struct UsageExporter {
    inner: Option<std::sync::Arc<dyn UsageExport>>,
}

impl UsageExporter {
    /// Disabled exporter: every publish attempt reports the refusal.
    pub fn disabled() -> Self {
        Self { inner: None }
    }

    pub fn with(export: std::sync::Arc<dyn UsageExport>) -> Self {
        Self {
            inner: Some(export),
        }
    }

    pub fn enabled(&self) -> bool {
        self.inner.is_some()
    }

    /// Fire-and-forget publish on a detached task — the per-record path.
    /// Errors are logged, never propagated.
    pub fn spawn_publish(&self, batch: UsageBatch) {
        let Some(export) = self.inner.clone() else {
            return;
        };
        tokio::spawn(async move {
            if let Err(error) = export.publish(batch).await {
                tracing::error!(%error, "usage event publish failed (best-effort)");
            }
        });
    }

    /// Awaited publish with the real outcome — the batch endpoint's path.
    pub async fn publish_now(&self, batch: UsageBatch) -> Result<usize, String> {
        let Some(export) = self.inner.as_ref() else {
            return Err("usage export is not configured".to_string());
        };
        let count = batch.records.len();
        export.publish(batch).await.map(|_| count)
    }
}
