use super::*;

struct PooledToolClient {
    inner: MockLlmClient,
    pool: crate::llm::ModelGenerationPool,
}

#[async_trait::async_trait]
impl LlmClient for PooledToolClient {
    async fn complete(
        &self,
        messages: &[Message],
        system: Option<&str>,
        tools: &[ToolDefinition],
    ) -> Result<LlmResponse> {
        self.inner.complete(messages, system, tools).await
    }

    async fn complete_streaming(
        &self,
        messages: &[Message],
        system: Option<&str>,
        tools: &[ToolDefinition],
        cancel: CancellationToken,
    ) -> Result<mpsc::Receiver<StreamEvent>> {
        self.inner
            .complete_streaming(messages, system, tools, cancel)
            .await
    }

    fn model_generation_pool(&self) -> Option<crate::llm::ModelGenerationPool> {
        Some(self.pool.clone())
    }
}

#[derive(Debug, Default)]
struct ParkReadHook {
    entered: Notify,
    release: Notify,
}

#[async_trait::async_trait]
impl crate::hooks::HookExecutor for ParkReadHook {
    async fn fire(&self, event: &crate::hooks::HookEvent) -> crate::hooks::HookResult {
        if matches!(event, crate::hooks::HookEvent::PreToolUse(event) if event.tool == "read") {
            self.entered.notify_one();
            self.release.notified().await;
        }
        crate::hooks::HookResult::Continue(None)
    }
}

#[tokio::test]
async fn background_tool_wait_releases_provider_capacity_for_foreground() {
    let workspace = tempfile::tempdir().unwrap();
    std::fs::write(workspace.path().join("alpha.txt"), "ALPHA").unwrap();
    let scheduler = Arc::new(
        crate::task_scheduler::TaskScheduler::new(crate::task_scheduler::TaskSchedulerConfig {
            max_active: 1,
            aging_interval_ms: 60_000,
        })
        .unwrap(),
    );
    let pool = crate::llm::ModelGenerationPool::for_endpoint(
        "test-provider",
        "test-model",
        "https://example.test/fairness",
        crate::llm::ModelGenerationConcurrency::single_flight(),
    )
    .unwrap();
    let client = Arc::new(PooledToolClient {
        pool: pool.clone(),
        inner: MockLlmClient::new(vec![
            MockLlmClient::tool_call_response(
                "child-read",
                "read",
                serde_json::json!({"file_path":"alpha.txt"}),
            ),
            MockLlmClient::text_response("ALPHA"),
        ]),
    });
    let hook = Arc::new(ParkReadHook::default());
    let parent = crate::child_run::ChildRunContext {
        security_provider: None,
        hook_engine: Some(hook.clone()),
        ..redacting_parent_context()
    };
    let executor = TaskExecutor::new(
        test_registry_with_writer(),
        client,
        workspace.path().to_str().unwrap().to_owned(),
    )
    .with_parent_context(parent)
    .with_task_scheduler(scheduler.clone(), false);
    let foreground = crate::llm::ModelGenerationAdmission::new(
        crate::llm::ModelGenerationConcurrency::single_flight(),
    )
    .with_model_generation_pool(
        scheduler.clone(),
        pool,
        crate::task_scheduler::TaskPriority::Foreground,
        "parent-generation",
    )
    .unwrap();
    let child = tokio::spawn(async move {
        executor
            .execute(
                TaskParams {
                    agent: "writer".into(),
                    description: "Background reader".into(),
                    prompt: "Read alpha.txt".into(),
                    background: true,
                    max_steps: Some(3),
                    output_schema: None,
                },
                None,
                Some("parent"),
            )
            .await
    });
    tokio::time::timeout(Duration::from_secs(5), hook.entered.notified())
        .await
        .expect("actual child read hook reached");
    let available = tokio::time::timeout(
        Duration::from_millis(250),
        foreground.acquire(&CancellationToken::new()),
    )
    .await;
    let admitted = matches!(&available, Ok(Ok(_)));
    drop(available);
    hook.release.notify_one();
    let result = tokio::time::timeout(Duration::from_secs(5), child)
        .await
        .unwrap()
        .unwrap()
        .unwrap();
    scheduler.shutdown().await;
    assert!(result.success, "{}", result.output);
    assert_eq!(result.output, "ALPHA");
    assert!(admitted, "a background tool wait must not reserve a single-flight provider for the entire child loop");
}
