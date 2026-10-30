use super::*;
use crate::llm::{LlmClient, LlmResponse, StreamEvent, ToolDefinition};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

#[derive(Default)]
struct PartialThenAnswer(AtomicUsize);

#[async_trait::async_trait]
impl LlmClient for PartialThenAnswer {
    async fn complete(
        &self,
        _: &[Message],
        _: Option<&str>,
        _: &[ToolDefinition],
    ) -> anyhow::Result<LlmResponse> {
        panic!("the controlled stream must not fall back")
    }

    async fn complete_streaming(
        &self,
        _: &[Message],
        _: Option<&str>,
        _: &[ToolDefinition],
        cancel: CancellationToken,
    ) -> anyhow::Result<mpsc::Receiver<StreamEvent>> {
        let (tx, rx) = mpsc::channel(2);
        if self.0.fetch_add(1, Ordering::SeqCst) == 0 {
            tx.try_send(StreamEvent::TextDelta("actual partial response".into()))
                .unwrap();
            tokio::spawn(async move {
                cancel.cancelled().await;
                drop(tx);
            });
        } else {
            tx.try_send(StreamEvent::Done(LlmResponse {
                message: Message::assistant("NEXT-ONLY"),
                usage: Default::default(),
                stop_reason: None,
                token_logprobs: Vec::new(),
                meta: None,
            }))
            .unwrap();
        }
        Ok(rx)
    }
}

#[tokio::test]
async fn cancelled_partial_model_has_no_completion_fact_and_next_turn_completes_once() {
    let workspace = tempfile::tempdir().unwrap();
    let client = Arc::new(PartialThenAnswer::default());
    let session = test_agent()
        .await
        .build_session(
            workspace.path().display().to_string(),
            client.clone(),
            &test_options("cancelled-model-completion"),
        )
        .unwrap();
    let (mut events, worker) = session
        .stream("original exact request", None)
        .await
        .unwrap();
    tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            match events.recv().await {
                Some(AgentEvent::TextDelta { text }) => {
                    assert_eq!(text, "actual partial response");
                    break;
                }
                Some(AgentEvent::End { .. } | AgentEvent::Error { .. }) | None => {
                    panic!("the original stream ended before cancellation")
                }
                _ => {}
            }
        }
    })
    .await
    .unwrap();
    let original = session.runs().await[0].id.clone();
    assert!(session.cancel().await);
    tokio::time::timeout(Duration::from_secs(5), async {
        while let Some(event) = events.recv().await {
            assert!(!matches!(event, AgentEvent::End { .. }));
        }
        worker.await.unwrap();
    })
    .await
    .unwrap();
    assert_eq!(session.runs().await[0].status, RunStatus::Cancelled);
    let facts = || {
        crate::fact_control::read_workspace_facts(
            workspace.path(),
            &crate::fact_control::thread_for_session(session.session_id()),
        )
        .unwrap()
    };
    assert!(
        !facts().iter().any(|fact| fact.kind == "turn.complete"),
        "a cancelled model response cannot settle a completed turn"
    );
    assert_eq!(
        session.send("next exact request", None).await.unwrap().text,
        "NEXT-ONLY"
    );
    let retained = facts();
    assert_eq!(
        retained
            .iter()
            .filter(|fact| fact.kind == "turn.complete")
            .count(),
        1
    );
    let users = retained
        .iter()
        .filter(|fact| fact.kind == "user.message")
        .map(|fact| fact.payload["text"].as_str().unwrap())
        .collect::<Vec<_>>();
    assert_eq!(users, ["original exact request", "next exact request"]);
    assert_eq!(client.0.load(Ordering::SeqCst), 2);
    assert_eq!(
        session.run_store.snapshot(&original).await.unwrap().status,
        RunStatus::Cancelled
    );
    session.close().await;
}
