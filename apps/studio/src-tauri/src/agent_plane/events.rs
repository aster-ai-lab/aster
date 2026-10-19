//! Ordered execution observations on the existing renderer event transport.

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

use a3s_boot::{SseEvent, SseStream};
use futures_util::StreamExt;
use serde_json::{Value, json};
use tokio::sync::broadcast;

const EVENT_BUFFER: usize = 2048;

pub struct EventHub {
    sender: broadcast::Sender<Value>,
    observations: Mutex<BTreeMap<String, Value>>,
    /// Last topic status. Only live statuses are copied onto the snapshot.
    statuses: Mutex<BTreeMap<String, Value>>,
}

fn live_status(status: &Value) -> bool {
    matches!(
        status.get("status").and_then(Value::as_str),
        Some("pending" | "streaming" | "awaiting-approval" | "awaiting-form")
    )
}

impl EventHub {
    pub fn new() -> Self {
        Self {
            sender: broadcast::channel(EVENT_BUFFER).0,
            observations: Mutex::new(BTreeMap::new()),
            statuses: Mutex::new(BTreeMap::new()),
        }
    }

    #[cfg(test)]
    pub fn subscribe(&self) -> broadcast::Receiver<Value> {
        self.sender.subscribe()
    }

    fn subscribe_snapshot(&self) -> (broadcast::Receiver<Value>, Value) {
        // observations then statuses. `publish` uses the same order.
        let observations = self
            .observations
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let statuses = self
            .statuses
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let receiver = self.sender.subscribe();
        let topics: Vec<_> = observations
            .iter()
            .map(|(topic, execution)| json!({"topicId":topic,"execution":execution}))
            .collect();
        let live_statuses: Vec<_> = statuses
            .iter()
            .filter(|(_, status)| live_status(status))
            .map(|(topic, status)| json!({"topicId":topic,"status":status}))
            .collect();
        (
            receiver,
            json!({"channel":"execution.snapshot","payload":{
                "protocolVersion":1,
                "topics":topics,
                "liveStatuses":live_statuses
            }}),
        )
    }

    pub(crate) fn publish(&self, channel: &str, payload: Value) {
        if channel == "topic.status" {
            // Publication and snapshot subscription share one ordering boundary.
            let mut observations = self
                .observations
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            let mut statuses = self
                .statuses
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            if let Some(topic) = payload.get("topicId").and_then(Value::as_str) {
                if let Some(execution) = payload
                    .get("status")
                    .and_then(|status| status.get("nativeExecution"))
                    .filter(|execution| !execution.is_null())
                {
                    observations.insert(topic.to_owned(), execution.clone());
                } else {
                    observations.remove(topic);
                }
                match payload.get("status").filter(|status| !status.is_null()) {
                    Some(status) if live_status(status) => {
                        statuses.insert(topic.to_owned(), status.clone());
                    }
                    _ => {
                        statuses.remove(topic);
                    }
                }
            }
            let _ = self
                .sender
                .send(json!({"channel":channel,"payload":payload}));
        } else {
            let _ = self
                .sender
                .send(json!({"channel":channel,"payload":payload}));
        }
    }
}

pub(super) fn stream(hub: Arc<EventHub>) -> SseStream {
    let (receiver, snapshot) = hub.subscribe_snapshot();
    futures_util::stream::unfold(
        (hub, receiver, Some(snapshot)),
        |(hub, mut receiver, pending)| async move {
            if let Some(snapshot) = pending {
                return Some((SseEvent::json(&snapshot), (hub, receiver, None)));
            }
            match receiver.recv().await {
                Ok(event) => Some((SseEvent::json(&event), (hub, receiver, None))),
                Err(broadcast::error::RecvError::Lagged(_)) => {
                    let (receiver, snapshot) = hub.subscribe_snapshot();
                    let invalidated =
                        json!({"channel":"execution.invalidated","payload":{"protocolVersion":1}});
                    Some((
                        SseEvent::json(&invalidated),
                        (hub, receiver, Some(snapshot)),
                    ))
                }
                Err(broadcast::error::RecvError::Closed) => None,
            }
        },
    )
    .boxed()
}

#[cfg(test)]
#[path = "events_tests.rs"]
mod tests;
