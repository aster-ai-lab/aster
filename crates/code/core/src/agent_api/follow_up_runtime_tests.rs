use super::*;
use std::sync::atomic::{AtomicUsize, Ordering};

struct FixtureClient {
    output: String,
    calls: AtomicUsize,
    hang: bool,
}

impl FixtureClient {
    fn new(output: &str) -> Self {
        Self {
            output: output.to_string(),
            calls: AtomicUsize::new(0),
            hang: false,
        }
    }
}

#[async_trait::async_trait]
impl LlmClient for FixtureClient {
    async fn complete(
        &self,
        _messages: &[Message],
        _system: Option<&str>,
        _tools: &[crate::llm::ToolDefinition],
    ) -> anyhow::Result<crate::llm::LlmResponse> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        if self.hang {
            std::future::pending::<()>().await;
        }
        Ok(crate::llm::LlmResponse {
            message: Message::assistant(&self.output),
            usage: Default::default(),
            stop_reason: None,
            token_logprobs: Vec::new(),
            meta: None,
        })
    }

    async fn complete_streaming(
        &self,
        _messages: &[Message],
        _system: Option<&str>,
        _tools: &[crate::llm::ToolDefinition],
        _cancel_token: tokio_util::sync::CancellationToken,
    ) -> anyhow::Result<mpsc::Receiver<crate::llm::StreamEvent>> {
        anyhow::bail!("follow-up generation must not open an agent stream")
    }
}

#[tokio::test]
async fn returns_bounded_deduplicated_questions_without_repair() {
    let client = FixtureClient::new(
        r#"{"primary":"需要我补充回归测试吗？","alternatives":["需要我补充回归测试吗？","要我检查边界情况吗？"]}"#,
    );
    let suggestions = generate_with_client(
        &client,
        "Fix the bug",
        "The bug is fixed",
        tokio_util::sync::CancellationToken::new(),
    )
    .await;
    assert_eq!(
        suggestions,
        Some(FollowUpSuggestions {
            primary: "需要我补充回归测试吗？".into(),
            alternatives: vec!["要我检查边界情况吗？".into()],
        })
    );
    assert_eq!(client.calls.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn invalid_output_suppresses_suggestions_without_another_call() {
    let client = FixtureClient::new(r#"{"primary":"   "}"#);
    let suggestions = generate_with_client(
        &client,
        "Fix the bug",
        "The bug is fixed",
        tokio_util::sync::CancellationToken::new(),
    )
    .await;
    assert_eq!(suggestions, None);
    assert_eq!(client.calls.load(Ordering::SeqCst), 1);
}

#[tokio::test(start_paused = true)]
async fn timeout_suppresses_suggestions_without_affecting_the_settled_turn() {
    let client = FixtureClient {
        output: String::new(),
        calls: AtomicUsize::new(0),
        hang: true,
    };
    let suggestions = generate_with_client(
        &client,
        "Fix the bug",
        "The bug is fixed",
        tokio_util::sync::CancellationToken::new(),
    )
    .await;
    assert_eq!(suggestions, None);
    assert_eq!(client.calls.load(Ordering::SeqCst), 1);
}
