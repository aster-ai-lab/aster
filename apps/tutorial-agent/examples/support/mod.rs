#![allow(dead_code)]

use a3s_code_core::llm::{StreamEvent, ToolDefinition};
use a3s_code_core::{Agent, CodeConfig, ContentBlock, LlmClient, LlmResponse, Message, TokenUsage};
use anyhow::{Context, Result};
use serde_json::Value;
use std::{
    collections::VecDeque,
    sync::{Arc, Mutex},
};
use tokio::sync::{Notify, mpsc};
use tokio_util::sync::CancellationToken;

pub struct Script {
    responses: Mutex<VecDeque<LlmResponse>>,
    pub inputs: Mutex<Vec<Vec<Message>>>,
    pub started: Notify,
    blocking: bool,
}

impl Script {
    pub fn new(responses: Vec<LlmResponse>) -> Arc<Self> {
        Arc::new(Self {
            responses: Mutex::new(responses.into()),
            inputs: Mutex::new(Vec::new()),
            started: Notify::new(),
            blocking: false,
        })
    }
    pub fn blocking() -> Arc<Self> {
        Arc::new(Self {
            responses: Mutex::new(VecDeque::new()),
            inputs: Mutex::new(Vec::new()),
            started: Notify::new(),
            blocking: true,
        })
    }
    fn next(&self, messages: &[Message]) -> Result<LlmResponse> {
        self.inputs.lock().unwrap().push(messages.to_vec());
        self.responses.lock().unwrap().pop_front().with_context(|| {
            format!(
                "固定响应已耗尽；最后输入：{}",
                serde_json::to_string(&messages.last())
                    .unwrap_or_default()
                    .chars()
                    .take(1200)
                    .collect::<String>()
            )
        })
    }
}

#[async_trait::async_trait]
impl LlmClient for Script {
    async fn complete(
        &self,
        messages: &[Message],
        _: Option<&str>,
        _: &[ToolDefinition],
    ) -> Result<LlmResponse> {
        self.next(messages)
    }
    async fn complete_streaming(
        &self,
        messages: &[Message],
        _: Option<&str>,
        _: &[ToolDefinition],
        cancellation: CancellationToken,
    ) -> Result<mpsc::Receiver<StreamEvent>> {
        self.started.notify_one();
        if self.blocking {
            cancellation.cancelled().await;
            anyhow::bail!("演示模型收到取消");
        }
        let response = self.next(messages)?;
        let (tx, rx) = mpsc::channel(4);
        tokio::spawn(async move {
            let text = response.text();
            if !text.is_empty() {
                let _ = tx.send(StreamEvent::TextDelta(text)).await;
            }
            let _ = tx.send(StreamEvent::Done(response)).await;
        });
        Ok(rx)
    }
}

fn response(content: Vec<ContentBlock>, stop_reason: &str) -> LlmResponse {
    LlmResponse {
        message: Message {
            role: "assistant".into(),
            content,
            reasoning_content: None,
            transcript_text: None,
            transcript_visibility: Default::default(),
        },
        usage: TokenUsage {
            prompt_tokens: 10,
            completion_tokens: 5,
            total_tokens: 15,
            cache_read_tokens: None,
            cache_write_tokens: None,
        },
        stop_reason: Some(stop_reason.into()),
        token_logprobs: Vec::new(),
        meta: None,
    }
}
pub fn text(value: &str) -> LlmResponse {
    response(vec![ContentBlock::Text { text: value.into() }], "end_turn")
}
pub fn tool(id: &str, name: &str, input: Value) -> LlmResponse {
    response(
        vec![ContentBlock::ToolUse {
            id: id.into(),
            name: name.into(),
            input,
        }],
        "tool_use",
    )
}
pub async fn agent() -> Result<Agent> {
    let mut config = CodeConfig::from_acl(
        r#"
        default_model = "openai/tutorial"
        providers "openai" {
          api_key = "fixed-response-no-network"
          models "tutorial" { name = "Offline tutorial fixture" }
        }
    "#,
    )?;
    config.memory = Some(a3s_code_core::memory::MemoryConfig {
        llm_extraction: false,
        ..Default::default()
    });
    Agent::from_config(config).await.map_err(Into::into)
}
