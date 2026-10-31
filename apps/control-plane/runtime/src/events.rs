//! In-memory recent-event ring for GET /v1/api/events/recent.

use chrono::Utc;
use serde::Serialize;
use serde_json::Value;

#[derive(Debug, Clone, Serialize)]
pub struct RuntimeEvent {
    pub at: String,
    pub kind: String,
    pub payload: Value,
}

pub struct EventRing {
    capacity: usize,
    items: Vec<RuntimeEvent>,
}

impl EventRing {
    pub fn new(capacity: usize) -> Self {
        Self {
            capacity: capacity.max(1),
            items: Vec::new(),
        }
    }

    pub fn push(&mut self, kind: impl Into<String>, payload: Value) {
        self.items.insert(
            0,
            RuntimeEvent {
                at: Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true),
                kind: kind.into(),
                payload,
            },
        );
        if self.items.len() > self.capacity {
            self.items.truncate(self.capacity);
        }
    }

    pub fn recent(&self, limit: usize) -> Vec<RuntimeEvent> {
        self.items.iter().take(limit).cloned().collect()
    }
}
