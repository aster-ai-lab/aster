use super::*;
use crate::agent_api::tests::test_config;
use crate::llm::{
    ContentBlock, LlmClient, LlmResponse, Message, StreamEvent, TokenUsage, ToolDefinition,
};
use crate::store::{MemorySessionStore, SessionStore};
use std::sync::{
    atomic::{AtomicBool, AtomicUsize, Ordering},
    Mutex,
};

#[derive(Clone)]
struct TitleClient {
    title: &'static str,
    calls: Arc<AtomicUsize>,
    caps: Arc<Mutex<Vec<u32>>>,
    forks: Arc<Mutex<Vec<String>>>,
}

#[async_trait::async_trait]
impl LlmClient for TitleClient {
    fn fork_for_session(&self, session_id: &str) -> Option<Arc<dyn LlmClient>> {
        self.forks.lock().unwrap().push(session_id.to_owned());
        Some(Arc::new(self.clone()))
    }

    fn with_output_cap(&self, cap: u32) -> Option<Arc<dyn LlmClient>> {
        self.caps.lock().unwrap().push(cap);
        Some(Arc::new(self.clone()))
    }

    async fn complete(
        &self,
        _: &[Message],
        _: Option<&str>,
        tools: &[ToolDefinition],
    ) -> anyhow::Result<LlmResponse> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        let (message, usage) = if tools.iter().any(|tool| tool.name == "session_title") {
            (
                Message {
                    role: "assistant".into(),
                    content: vec![ContentBlock::ToolUse {
                        id: "title".into(),
                        name: "session_title".into(),
                        input: serde_json::json!({"session_title":self.title}),
                    }],
                    reasoning_content: None,
                    transcript_text: None,
                    transcript_visibility: crate::llm::TranscriptVisibility::Wire,
                },
                TokenUsage {
                    prompt_tokens: 5,
                    completion_tokens: 2,
                    total_tokens: 7,
                    ..Default::default()
                },
            )
        } else {
            (
                Message::assistant("Primary response"),
                TokenUsage {
                    prompt_tokens: 17,
                    completion_tokens: 13,
                    total_tokens: 30,
                    ..Default::default()
                },
            )
        };
        Ok(LlmResponse {
            message,
            usage,
            stop_reason: None,
            token_logprobs: vec![],
            meta: None,
        })
    }

    async fn complete_streaming(
        &self,
        messages: &[Message],
        system: Option<&str>,
        tools: &[ToolDefinition],
        _: tokio_util::sync::CancellationToken,
    ) -> anyhow::Result<tokio::sync::mpsc::Receiver<StreamEvent>> {
        let response = self.complete(messages, system, tools).await?;
        let (tx, rx) = tokio::sync::mpsc::channel(1);
        tx.send(StreamEvent::Done(response)).await.unwrap();
        Ok(rx)
    }
}

#[derive(Default)]
struct TitleBudget {
    deny: AtomicBool,
    checks: Mutex<Vec<String>>,
    records: Mutex<Vec<(String, usize)>>,
}

#[async_trait::async_trait]
impl crate::budget::BudgetGuard for TitleBudget {
    async fn check_before_llm(&self, session_id: &str, _: usize) -> crate::budget::BudgetDecision {
        self.checks.lock().unwrap().push(session_id.to_owned());
        if self.deny.load(Ordering::SeqCst) {
            crate::budget::BudgetDecision::Deny {
                resource: "tokens".into(),
                reason: "title budget exhausted".into(),
            }
        } else {
            crate::budget::BudgetDecision::Allow
        }
    }

    async fn record_after_llm(&self, session_id: &str, usage: &TokenUsage) {
        self.records
            .lock()
            .unwrap()
            .push((session_id.to_owned(), usage.total_tokens));
    }
}

#[tokio::test]
async fn title_uses_original_budget_and_saves_reported_usage_even_when_rejected() {
    for (title, deny) in [
        ("A useful title for the current workspace", false),
        ("", false),
        ("Denied title", true),
    ] {
        let root = tempfile::tempdir().unwrap();
        let client = Arc::new(TitleClient {
            title,
            calls: Default::default(),
            caps: Default::default(),
            forks: Default::default(),
        });
        let budget = Arc::new(TitleBudget::default());
        let store = Arc::new(MemorySessionStore::new());
        let mut config = test_config();
        config.memory = Some(crate::memory::MemoryConfig {
            llm_extraction: false,
            ..Default::default()
        });
        let agent = Agent::from_config(config).await.unwrap();
        let session = agent
            .build_session(
                root.path().display().to_string(),
                client.clone(),
                &SessionOptions::new()
                    .with_session_id("title-usage")
                    .with_session_store(store.clone())
                    .with_auto_save(true)
                    .with_budget_guard(budget.clone())
                    .with_planning_mode(crate::PlanningMode::Disabled),
            )
            .unwrap();
        session.send("Explain this workspace", None).await.unwrap();
        assert_eq!(session.recorded_usage_tokens(), (17, 13, 30));
        budget.deny.store(deny, Ordering::SeqCst);
        let actual = session
            .generate_session_title("Explain this workspace")
            .await;
        assert_eq!(
            actual.as_deref(),
            if title.is_empty() || deny {
                None
            } else {
                Some(title)
            }
        );
        assert_eq!(
            budget.checks.lock().unwrap().as_slice(),
            ["title-usage", "title-usage"]
        );
        let expected_records = if deny {
            vec![("title-usage".into(), 30)]
        } else {
            vec![("title-usage".into(), 30), ("title-usage".into(), 7)]
        };
        assert_eq!(*budget.records.lock().unwrap(), expected_records);
        assert_eq!(
            client.calls.load(Ordering::SeqCst),
            if deny { 1 } else { 2 }
        );
        assert!(client
            .forks
            .lock()
            .unwrap()
            .iter()
            .any(|id| id == "title-usage:session-title"));
        assert!(client.caps.lock().unwrap().contains(&100));
        assert_eq!(
            session.recorded_usage_tokens(),
            if deny { (17, 13, 30) } else { (22, 15, 37) }
        );
        let saved = store.load_snapshot("title-usage").await.unwrap().unwrap();
        assert_eq!(
            saved.session.total_usage.total_tokens,
            if deny { 30 } else { 37 }
        );
        assert_eq!(saved.run_records.len(), 1);
        session.close().await;
        agent.close().await;
    }
}
