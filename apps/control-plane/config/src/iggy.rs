//! Iggy bus for config-center fan-out.
//!
//! One stream (`aster_config`) and one topic (`config_changed`). Each process
//! polls from the head with its own consumer, so every instance sees each
//! change. A consumer group would deliver a change to only one instance.

use std::sync::Arc;
use std::time::Duration;

use aster_boot::{BootError, Result};
use iggy::clients::client::IggyClient as SdkClient;
use iggy::clients::client_builder::IggyClientBuilder;
use iggy::prelude::{
    Consumer, Identifier, IggyDuration, IggyExpiry, IggyMessage, MessageClient, Partitioning,
    PollingStrategy, StreamClient, TopicClient, TopicCreateOptions, UserClient,
};
use serde::{de::DeserializeOwned, Serialize};

/// Logical name of the config-change channel. Transport is the Iggy topic.
pub const CONFIG_CHANGED_SUBJECT: &str = "aster.config.changed";

const STREAM_NAME: &str = "aster_config";
const TOPIC_NAME: &str = "config_changed";
const HEADER_SUBJECT: &str = "aster-subject";

/// Login and address for the dev Iggy broker (`aster-iggy`, TCP :5102).
#[derive(Debug, Clone)]
pub struct IggyEndpoint {
    pub address: String,
    pub username: String,
    pub password: String,
}

pub struct IggyBus {
    client: Arc<SdkClient>,
    address: String,
    stream: Identifier,
    topic: Identifier,
}

impl std::fmt::Debug for IggyBus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("IggyBus")
            .field("address", &self.address)
            .field("stream", &STREAM_NAME)
            .field("topic", &TOPIC_NAME)
            .finish_non_exhaustive()
    }
}

impl IggyBus {
    pub async fn connect(endpoint: &IggyEndpoint) -> Result<Self> {
        let address = endpoint.address.clone();
        let username = endpoint.username.clone();
        let password = endpoint.password.clone();
        let connect_fut = async move {
            let sdk = IggyClientBuilder::new()
                .with_tcp()
                .with_server_address(address.clone())
                .build()
                .map_err(|err| BootError::Internal(format!("iggy client {address}: {err}")))?;
            sdk.login_user(&username, &password).await.map_err(|err| {
                BootError::Internal(format!("iggy login failed for '{username}': {err}"))
            })?;
            Ok::<SdkClient, BootError>(sdk)
        };
        let sdk = tokio::time::timeout(Duration::from_secs(5), connect_fut)
            .await
            .map_err(|_| {
                BootError::Internal(format!(
                    "iggy connect to {} timed out after 5s",
                    endpoint.address
                ))
            })??;

        let stream = Identifier::named(STREAM_NAME)
            .map_err(|err| BootError::Internal(format!("invalid iggy stream name: {err}")))?;
        ensure_stream(&sdk, &stream).await?;
        let topic = Identifier::named(TOPIC_NAME)
            .map_err(|err| BootError::Internal(format!("invalid iggy topic name: {err}")))?;
        ensure_topic(&sdk, &stream, &topic).await?;

        Ok(Self {
            client: Arc::new(sdk),
            address: endpoint.address.clone(),
            stream,
            topic,
        })
    }

    pub fn address(&self) -> &str {
        &self.address
    }

    pub async fn ping(&self) -> Result<()> {
        self.client
            .get_stream(&self.stream)
            .await
            .map_err(|err| BootError::Internal(format!("iggy health failed: {err}")))?
            .ok_or_else(|| BootError::Internal(format!("iggy stream {STREAM_NAME} missing")))?;
        Ok(())
    }

    pub async fn publish_json<T: Serialize>(&self, subject: &str, payload: &T) -> Result<()> {
        let bytes = serde_json::to_vec(payload)
            .map_err(|err| BootError::Internal(format!("iggy encode failed: {err}")))?;
        let mut headers = std::collections::BTreeMap::new();
        if let (Ok(key), Ok(value)) = (HEADER_SUBJECT.parse(), subject.parse()) {
            headers.insert(key, value);
        }
        let message = IggyMessage::builder()
            .payload(bytes::Bytes::from(bytes))
            .user_headers(headers)
            .build()
            .map_err(|err| BootError::Internal(format!("iggy message build failed: {err}")))?;
        let mut messages = [message];
        self.client
            .send_messages(
                &self.stream,
                &self.topic,
                &Partitioning::partition_id(0),
                &mut messages,
            )
            .await
            .map_err(|err| BootError::Internal(format!("iggy publish failed: {err}")))?;
        Ok(())
    }

    /// Deliver JSON messages published after this call. Does not replay history.
    pub async fn subscribe_json_loop<T, F, Fut>(&self, mut handler: F) -> Result<()>
    where
        T: DeserializeOwned,
        F: FnMut(T) -> Fut,
        Fut: std::future::Future<Output = ()>,
    {
        let mut next = self.head_offset().await?;
        let consumer = fanout_consumer();
        loop {
            let polled = match tokio::time::timeout(
                Duration::from_secs(2),
                self.client.poll_messages(
                    &self.stream,
                    &self.topic,
                    Some(0),
                    &consumer,
                    &PollingStrategy::offset(next),
                    32,
                    false,
                ),
            )
            .await
            {
                Ok(Ok(polled)) => polled,
                Ok(Err(err)) => {
                    eprintln!("iggy poll failed on {TOPIC_NAME}: {err}");
                    tokio::time::sleep(Duration::from_secs(1)).await;
                    continue;
                }
                Err(_) => {
                    tokio::time::sleep(Duration::from_millis(50)).await;
                    continue;
                }
            };
            if polled.messages.is_empty() {
                tokio::time::sleep(Duration::from_millis(50)).await;
                continue;
            }
            for message in &polled.messages {
                next = message.header.offset.saturating_add(1);
                match serde_json::from_slice::<T>(&message.payload) {
                    Ok(payload) => handler(payload).await,
                    Err(err) => eprintln!("iggy decode ignored on {TOPIC_NAME}: {err}"),
                }
            }
        }
    }

    async fn head_offset(&self) -> Result<u64> {
        let polled = tokio::time::timeout(
            Duration::from_secs(5),
            self.client.poll_messages(
                &self.stream,
                &self.topic,
                Some(0),
                &fanout_consumer(),
                &PollingStrategy::last(),
                1,
                false,
            ),
        )
        .await
        .map_err(|_| BootError::Internal("iggy head probe timed out".into()))?
        .map_err(|err| BootError::Internal(format!("iggy head probe failed: {err}")))?;
        Ok(polled.current_offset)
    }
}

fn fanout_consumer() -> Consumer {
    let id = std::process::id()
        ^ (std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.subsec_nanos())
            .unwrap_or(1));
    Consumer::new(Identifier::numeric(id).unwrap_or_else(|_| Identifier::numeric(1).unwrap()))
}

async fn ensure_stream(sdk: &SdkClient, stream: &Identifier) -> Result<()> {
    match sdk.get_stream(stream).await {
        Ok(Some(_)) => return Ok(()),
        Ok(None) => {}
        Err(err) if is_not_found(&err) => {}
        Err(err) => {
            return Err(BootError::Internal(format!(
                "iggy inspect stream {STREAM_NAME} failed: {err}"
            )))
        }
    }
    sdk.create_stream(STREAM_NAME)
        .await
        .map(|_| ())
        .or_else(|err| {
            if is_already_exists(&err) {
                Ok(())
            } else {
                Err(BootError::Internal(format!(
                    "iggy create stream {STREAM_NAME} failed: {err}"
                )))
            }
        })
}

async fn ensure_topic(sdk: &SdkClient, stream: &Identifier, topic: &Identifier) -> Result<()> {
    match sdk.get_topic(stream, topic).await {
        Ok(Some(_)) => return Ok(()),
        Ok(None) => {}
        Err(err) if is_not_found(&err) => {}
        Err(err) => {
            return Err(BootError::Internal(format!(
                "iggy inspect topic {TOPIC_NAME} failed: {err}"
            )))
        }
    }
    let options = TopicCreateOptions {
        partitions_count: Some(1),
        message_expiry: Some(IggyExpiry::ExpireDuration(IggyDuration::from(
            Duration::from_secs(86_400),
        ))),
        ..Default::default()
    };
    sdk.create_topic(stream, TOPIC_NAME, &options)
        .await
        .map(|_| ())
        .or_else(|err| {
            if is_already_exists(&err) {
                Ok(())
            } else {
                Err(BootError::Internal(format!(
                    "iggy create topic {TOPIC_NAME} failed: {err}"
                )))
            }
        })
}

fn is_already_exists(err: &iggy::prelude::IggyError) -> bool {
    matches!(
        err,
        iggy::prelude::IggyError::StreamNameAlreadyExists(_)
            | iggy::prelude::IggyError::TopicNameAlreadyExists(..)
    )
}

fn is_not_found(err: &iggy::prelude::IggyError) -> bool {
    matches!(
        err,
        iggy::prelude::IggyError::ResourceNotFound(_)
            | iggy::prelude::IggyError::StreamIdNotFound(_)
            | iggy::prelude::IggyError::TopicIdNotFound(..)
    )
}
