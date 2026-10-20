//! Durable L1 usage facts — append-only jsonl under A3S_FUNCTION_DATA_DIR.

use std::path::{Path, PathBuf};

use chrono::Utc;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum UsageStatus {
    Ok,
    Denied,
    Error,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UsageRecord {
    pub id: String,
    pub at: String,
    pub request_id: String,
    pub sku: String,
    pub listing_kind: String,
    pub credential_prefix: String,
    pub units: f64,
    pub unit_price_usd: f64,
    pub amount_usd: f64,
    pub status: UsageStatus,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
    pub route: String,
    pub org_id: String,
    pub project_id: String,
    pub env_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub duration_ms: Option<u64>,
}

pub struct UsageInput {
    pub request_id: String,
    pub sku: String,
    pub listing_kind: String,
    pub credential_prefix: String,
    pub units: f64,
    pub unit_price_usd: f64,
    pub status: UsageStatus,
    pub detail: Option<String>,
    pub route: String,
    pub org_id: String,
    pub project_id: String,
    pub env_id: String,
    pub duration_ms: Option<u64>,
}

pub struct UsageStore {
    path: PathBuf,
    rows: Vec<UsageRecord>,
}

impl UsageStore {
    pub fn open(data_dir: &Path) -> Self {
        let _ = std::fs::create_dir_all(data_dir);
        let path = data_dir.join("usage.jsonl");
        let rows = load_all(&path);
        Self { path, rows }
    }

    pub fn list(&self, limit: usize) -> Vec<UsageRecord> {
        let mut sorted = self.rows.clone();
        sorted.sort_by(|a, b| b.at.cmp(&a.at));
        sorted.into_iter().take(limit).collect()
    }

    pub fn spend_for_prefix(&self, prefix: &str) -> f64 {
        self.rows
            .iter()
            .filter(|r| r.credential_prefix == prefix && r.status == UsageStatus::Ok)
            .map(|r| r.amount_usd)
            .sum()
    }

    /// One accepted quantity. The same request id does not append a second row.
    pub fn record_once(&mut self, input: UsageInput) -> UsageRecord {
        if let Some(existing) = self.rows.iter().find(|row| {
            row.request_id == input.request_id
                && row.sku == input.sku
                && row.status == UsageStatus::Ok
        }) {
            return existing.clone();
        }
        self.record(input)
    }

    pub fn record(&mut self, input: UsageInput) -> UsageRecord {
        let amount_usd = ((input.units * input.unit_price_usd) * 1_000_000.0).round() / 1_000_000.0;
        let row = UsageRecord {
            id: format!(
                "usage_{}_{}",
                Utc::now().timestamp_millis().to_string(),
                &Uuid::new_v4().to_string()[..5]
            ),
            at: Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true),
            request_id: input.request_id,
            sku: input.sku,
            listing_kind: input.listing_kind,
            credential_prefix: input.credential_prefix,
            units: input.units,
            unit_price_usd: input.unit_price_usd,
            amount_usd,
            status: input.status,
            detail: input.detail,
            route: input.route,
            org_id: input.org_id,
            project_id: input.project_id,
            env_id: input.env_id,
            duration_ms: input.duration_ms,
        };
        self.rows.insert(0, row.clone());
        if let Ok(line) = serde_json::to_string(&row) {
            use std::io::Write;
            if let Ok(mut f) = std::fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(&self.path)
            {
                let _ = writeln!(f, "{line}");
            }
        }
        row
    }
}

fn load_all(path: &Path) -> Vec<UsageRecord> {
    let Ok(text) = std::fs::read_to_string(path) else {
        return Vec::new();
    };
    text.lines()
        .filter(|l| !l.is_empty())
        .filter_map(|line| serde_json::from_str(line).ok())
        .collect()
}

/// Quantity recorded for one accepted platform delivery.
pub const ACCEPTED_UNITS: f64 = 1.0;

pub fn data_dir() -> PathBuf {
    if let Ok(dir) = std::env::var("A3S_FUNCTION_DATA_DIR") {
        return PathBuf::from(dir);
    }
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(".data")
}
