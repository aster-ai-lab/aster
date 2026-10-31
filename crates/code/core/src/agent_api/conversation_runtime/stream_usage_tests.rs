#![cfg(test)]

use super::*;

#[tokio::test]
async fn public_stream_end_preserves_mapped_usage_including_cache_tokens() {
    let result = AgentResult {
        text: "source analysis complete".into(),
        messages: Vec::new(),
        usage: crate::llm::TokenUsage {
            prompt_tokens: 701,
            completion_tokens: 43,
            total_tokens: 744,
            cache_read_tokens: Some(512),
            cache_write_tokens: Some(128),
        },
        tool_calls_count: 0,
        verification_reports: Vec::new(),
        completion: crate::harness_loop::CompletionTerminal::Narrative,
        forced_stop: None,
        run_admission: "ordinary".into(),
    };
    let mut view = aster_effect::CodingView::empty();
    view.assistant = Some(result.text.clone());
    let settled = Ok(aster_effect::Settlement {
        view,
        log: Vec::new(),
        steps: 0,
        cut: aster_effect::cut_log(&[]),
        trace: Vec::new(),
    });
    for streamed_text in [false, true] {
        let (tx, mut rx) = mpsc::channel(4);
        emit_settled(&tx, &settled, Some(&result), streamed_text).await;
        drop(tx);
        let mut ends = 0;
        let mut deltas = 0;
        while let Some(event) = rx.recv().await {
            match event {
                AgentEvent::TextDelta { text } => {
                    assert_eq!(text, result.text);
                    deltas += 1;
                }
                AgentEvent::End { text, usage, .. } => {
                    assert_eq!(text, result.text);
                    assert_eq!(
                        serde_json::to_value(usage).unwrap(),
                        serde_json::to_value(&result.usage).unwrap()
                    );
                    ends += 1;
                }
                other => panic!("unexpected public settlement event: {other:?}"),
            }
        }
        assert_eq!(ends, 1);
        assert_eq!(deltas, usize::from(!streamed_text));
    }
}

struct TimedResponses(std::sync::Mutex<std::collections::VecDeque<crate::llm::LlmResponse>>);

#[async_trait::async_trait]
impl crate::llm::LlmClient for TimedResponses {
    async fn complete(
        &self,
        _: &[Message],
        _: Option<&str>,
        _: &[crate::llm::ToolDefinition],
    ) -> anyhow::Result<crate::llm::LlmResponse> {
        panic!("the streaming fixture must not fall back")
    }
    async fn complete_streaming(
        &self,
        _: &[Message],
        _: Option<&str>,
        _: &[crate::llm::ToolDefinition],
        _: tokio_util::sync::CancellationToken,
    ) -> anyhow::Result<mpsc::Receiver<crate::llm::StreamEvent>> {
        let response = self
            .0
            .lock()
            .unwrap()
            .pop_front()
            .expect("unexpected model call");
        let (tx, rx) = mpsc::channel(2);
        tx.send(crate::llm::StreamEvent::TextDelta(response.text()))
            .await
            .unwrap();
        tx.send(crate::llm::StreamEvent::Done(response))
            .await
            .unwrap();
        Ok(rx)
    }
}

#[tokio::test]
async fn public_stream_and_recorded_end_preserve_provider_timing_without_leaking_between_runs() {
    let workspace = tempfile::tempdir().unwrap();
    let mut config = super::super::test_config();
    config.memory = Some(crate::memory::MemoryConfig {
        llm_extraction: false,
        prune_policy: None,
        ..Default::default()
    });
    let agent = super::super::Agent::from_config(config).await.unwrap();
    let measured = crate::llm::LlmResponseMeta {
        response_model: Some("returned-model".into()),
        first_token_ms: Some(250),
        duration_ms: Some(1250),
        ..Default::default()
    };
    let replies = [Some(measured.clone()), None].map(|meta| crate::llm::LlmResponse {
        message: Message::assistant("The requested explanation is complete."),
        usage: crate::llm::TokenUsage {
            prompt_tokens: 17,
            completion_tokens: 13,
            total_tokens: 30,
            ..Default::default()
        },
        stop_reason: Some("end_turn".into()),
        token_logprobs: vec![],
        meta,
    });
    let client = Arc::new(TimedResponses(std::sync::Mutex::new(replies.into())));
    let session = agent
        .build_session(
            workspace.path().display().to_string(),
            client,
            &super::super::SessionOptions::new()
                .with_planning(false)
                .with_auto_delegation_enabled(false)
                .with_manual_delegation_enabled(false),
        )
        .unwrap();
    for expected in [Some(measured), None] {
        let mut stream = session
            .stream_with_run("Explain a fixed fact without tools.", None)
            .await
            .unwrap();
        let run = stream.run.take().unwrap();
        let mut ends = 0;
        tokio::time::timeout(std::time::Duration::from_secs(5), async {
            while let Some(event) = stream.events.recv().await {
                if let AgentEvent::End { meta, usage, .. } = event {
                    assert_eq!(
                        serde_json::to_value(meta).unwrap(),
                        serde_json::to_value(&expected).unwrap()
                    );
                    assert_eq!(usage.completion_tokens, 13);
                    ends += 1;
                }
            }
        })
        .await
        .unwrap();
        stream.worker.await.unwrap();
        assert_eq!(ends, 1);
        let events = run.events().await;
        let stored = events
            .iter()
            .find_map(|entry| match &entry.event {
                AgentEvent::End { meta, .. } => Some(meta),
                _ => None,
            })
            .unwrap();
        assert_eq!(
            serde_json::to_value(stored).unwrap(),
            serde_json::to_value(expected).unwrap()
        );
    }
}
