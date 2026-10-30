//! Per-turn coalescing is bounded and holds no prompt or caller secret.
use super::decision_cache::TurnIdentity;
use std::collections::HashMap;
use std::sync::{Arc, Mutex, Weak};

const CAPACITY: usize = 4096;
#[derive(Default)]
pub(super) struct DecisionFlights {
    gates: Mutex<HashMap<TurnIdentity, Weak<tokio::sync::Mutex<()>>>>,
}
impl DecisionFlights {
    pub(super) async fn acquire(
        &self,
        key: TurnIdentity,
    ) -> Option<tokio::sync::OwnedMutexGuard<()>> {
        let gate = {
            let mut gates = self.gates.lock().ok()?;
            if let Some(gate) = gates.get(&key).and_then(Weak::upgrade) {
                gate
            } else {
                gates.retain(|_, gate| gate.strong_count() > 0);
                if gates.len() >= CAPACITY {
                    return None;
                }
                let gate = Arc::new(tokio::sync::Mutex::new(()));
                gates.insert(key, Arc::downgrade(&gate));
                gate
            }
        };
        tokio::time::timeout(std::time::Duration::from_secs(6), gate.lock_owned())
            .await
            .ok()
    }
}
