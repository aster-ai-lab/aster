use super::*;
use aster_core::llm::{LlmClient, LlmResponse, Message, StreamEvent, TokenUsage, ToolDefinition};
use tokio::io::AsyncBufReadExt;
use tokio_util::compat::{TokioAsyncReadCompatExt, TokioAsyncWriteCompatExt};

struct ReportedClient;

#[async_trait::async_trait]
impl LlmClient for ReportedClient {
    async fn complete(
        &self,
        _: &[Message],
        _: Option<&str>,
        _: &[ToolDefinition],
    ) -> anyhow::Result<LlmResponse> {
        Ok(LlmResponse {
            message: Message::assistant("Complete"),
            usage: TokenUsage {
                prompt_tokens: 5,
                completion_tokens: 2,
                total_tokens: 7,
                ..Default::default()
            },
            stop_reason: None,
            token_logprobs: vec![],
            meta: None,
        })
    }
    async fn complete_streaming(
        &self,
        _: &[Message],
        _: Option<&str>,
        _: &[ToolDefinition],
        _: tokio_util::sync::CancellationToken,
    ) -> anyhow::Result<tokio::sync::mpsc::Receiver<StreamEvent>> {
        anyhow::bail!("non-streaming fixture")
    }
}

#[tokio::test]
async fn session_usage_notifications_follow_core_calls_outside_an_acp_prompt_without_retaining_client(
) {
    tokio::task::LocalSet::new()
        .run_until(async {
            let root = tempfile::tempdir().unwrap();
            let mut config = aster_core::CodeConfig::default();
            config.memory = Some(aster_core::memory::MemoryConfig {
                llm_extraction: false,
                ..Default::default()
            });
            let core = aster_core::Agent::from_config(config).await.unwrap();
            let session = core
                .session_async(
                    root.path().display().to_string(),
                    Some(
                        aster_core::SessionOptions::new()
                            .with_llm_client(Arc::new(ReportedClient))
                            .with_planning_mode(aster_core::PlanningMode::Disabled),
                    ),
                )
                .await
                .unwrap();
            let agent = AsterCodeAgent::new(crate::agent::tests::test_launch(None));
            let (client_end, agent_end) = tokio::io::duplex(64 * 1024);
            let (read, write) = tokio::io::split(agent_end);
            let (connection, io) = acp::AgentSideConnection::new(
                AsterCodeAgent::new(crate::agent::tests::test_launch(None)),
                write.compat_write(),
                read.compat(),
                |future| {
                    tokio::task::spawn_local(future);
                },
            );
            let transport = tokio::task::spawn_local(io);
            let connection = Rc::new(connection);
            let weak = Rc::downgrade(&connection);
            agent.set_client(connection.clone());
            drop(connection);
            let mut lines = tokio::io::BufReader::new(client_end).lines();
            agent.announce_session_usage(session.session_id(), &session);
            for expected in [0, 7, 14] {
                if expected > 0 {
                    session.send("Report one call", None).await.unwrap();
                }
                let line =
                    tokio::time::timeout(std::time::Duration::from_secs(1), lines.next_line())
                        .await
                        .unwrap()
                        .unwrap()
                        .unwrap();
                let packet: serde_json::Value = serde_json::from_str(&line).unwrap();
                assert_eq!(packet["method"], "_aster/session_notification");
                assert_eq!(packet["params"]["sessionId"], session.session_id());
                assert_eq!(packet["params"]["update"]["sessionUpdate"], "model_info");
                assert_eq!(packet["params"]["update"]["sessionTotalTokens"], expected);
            }
            agent.client.borrow_mut().take();
            tokio::task::yield_now().await;
            assert!(weak.upgrade().is_none());
            session.close().await;
            core.close().await;
            transport.abort();
            let _ = transport.await;
        })
        .await;
}
