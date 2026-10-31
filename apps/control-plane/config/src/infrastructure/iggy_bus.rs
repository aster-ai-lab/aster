//! Iggy adapter for the config-center change bus.

use std::sync::Arc;

use async_trait::async_trait;
use tokio::sync::mpsc;

use crate::iggy::IggyBus;

use super::super::application::{ConfigChangeBus, ConfigChangedEvent};

pub struct IggyConfigChangeBus {
    iggy: Arc<IggyBus>,
}

impl IggyConfigChangeBus {
    pub fn new(iggy: Arc<IggyBus>) -> Self {
        Self { iggy }
    }
}

#[async_trait]
impl ConfigChangeBus for IggyConfigChangeBus {
    async fn publish(&self, event: &ConfigChangedEvent) -> Result<(), String> {
        self.iggy
            .publish_json(crate::CONFIG_CHANGED_SUBJECT, event)
            .await
            .map_err(|err| err.to_string())
    }

    fn spawn_listen(&self, tx: mpsc::Sender<ConfigChangedEvent>) {
        let iggy = Arc::clone(&self.iggy);
        tokio::spawn(async move {
            let result = iggy
                .subscribe_json_loop(|event: ConfigChangedEvent| {
                    let tx = tx.clone();
                    async move {
                        let _ = tx.send(event).await;
                    }
                })
                .await;
            if let Err(err) = result {
                eprintln!("config center iggy subscriber stopped: {err}");
            }
        });
    }
}
