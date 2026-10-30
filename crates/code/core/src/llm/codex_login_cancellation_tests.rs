#![cfg(test)]

use super::tests::{codex_client, ScriptedCodexSse};
use super::*;
use std::time::Duration;

struct RefusedHttp(u16);

#[async_trait]
impl HttpClient for RefusedHttp {
    async fn post(
        &self,
        _: &str,
        _: Vec<(&str, &str)>,
        _: &Value,
        _: CancellationToken,
    ) -> Result<crate::llm::HttpResponse> {
        unreachable!("Codex must use the Responses stream")
    }

    async fn post_streaming(
        &self,
        url: &str,
        headers: Vec<(&str, &str)>,
        body: &Value,
        _: CancellationToken,
    ) -> Result<crate::llm::StreamingHttpResponse> {
        assert_eq!(url, "https://chatgpt.com/backend-api/codex/responses");
        assert!(headers.contains(&("chatgpt-account-id", "acct")));
        assert_eq!(body["stream"], true);
        assert_eq!(body["store"], false);
        Ok(crate::llm::StreamingHttpResponse {
            status: self.0,
            retry_after: None,
            byte_stream: Box::pin(futures::stream::empty()),
            error_body: "synthetic-secret private-account".into(),
        })
    }
}

#[tokio::test]
async fn codex_preserves_http_status_without_turning_rate_limits_into_terminal_errors() {
    for status in [401, 429, 503] {
        let error = codex_client(Arc::new(RefusedHttp(status)))
            .complete(&[Message::user("OK")], None, &[])
            .await
            .unwrap_err();
        if status == 401 {
            assert_eq!(
                error
                    .downcast_ref::<crate::llm::NonRetryableLlmError>()
                    .unwrap()
                    .status(),
                Some(401)
            );
        } else {
            assert_eq!(
                error.downcast_ref::<CodexHttpError>().unwrap().status(),
                status
            );
            assert!(error
                .downcast_ref::<crate::llm::NonRetryableLlmError>()
                .is_none());
        }
    }
}

#[tokio::test]
async fn dropping_complete_closes_the_pending_provider_stream() {
    let (tx, rx) = tokio::sync::mpsc::unbounded_channel();
    let parked = Arc::new(tokio::sync::Notify::new());
    let client = codex_client(Arc::new(ScriptedCodexSse {
        incoming: std::sync::Mutex::new(Some(rx)),
        parked: Arc::clone(&parked),
    }));
    let task =
        tokio::spawn(async move { client.complete(&[Message::user("OK")], None, &[]).await });
    tx.send(Some(bytes::Bytes::from_static(b"data: {}\n\n")))
        .unwrap();
    tokio::time::timeout(Duration::from_secs(1), parked.notified())
        .await
        .unwrap();
    task.abort();
    assert!(task.await.unwrap_err().is_cancelled());
    tokio::time::timeout(Duration::from_secs(1), tx.closed())
        .await
        .expect("a timed-out completion must release its live stream");
}

#[tokio::test]
async fn cancellation_closes_a_silent_codex_stream() {
    let (tx, rx) = tokio::sync::mpsc::unbounded_channel();
    let client = codex_client(Arc::new(ScriptedCodexSse {
        incoming: std::sync::Mutex::new(Some(rx)),
        parked: Arc::new(tokio::sync::Notify::new()),
    }));
    let cancel = CancellationToken::new();
    let mut events = client
        .complete_streaming(&[Message::user("OK")], None, &[], cancel.clone())
        .await
        .unwrap();
    cancel.cancel();
    tokio::time::timeout(Duration::from_secs(1), tx.closed())
        .await
        .expect("cancellation must drop the provider stream even without another chunk");
    assert!(events.recv().await.is_none());
}

#[tokio::test]
async fn generation_deadlines_and_session_forks_preserve_the_owned_http_transport() {
    let client = codex_client(Arc::new(RefusedHttp(401)));
    let forked = client.fork_for_session("child-session").unwrap();
    let scoped = forked
        .with_active_generation_timeout(Duration::from_secs(1))
        .unwrap_or(forked);
    let error = scoped
        .complete(&[Message::user("OK")], None, &[])
        .await
        .unwrap_err();
    assert_eq!(
        error
            .downcast_ref::<crate::llm::NonRetryableLlmError>()
            .unwrap()
            .status(),
        Some(401)
    );
}

#[tokio::test]
async fn responses_decoder_keeps_utf8_split_across_single_byte_transport_chunks() {
    let (tx, rx) = tokio::sync::mpsc::unbounded_channel();
    let client = codex_client(Arc::new(ScriptedCodexSse {
        incoming: std::sync::Mutex::new(Some(rx)),
        parked: Arc::new(tokio::sync::Notify::new()),
    }));
    let input = format!(
        "data: {}\r\n\r\ndata: {}\r\n\r\n",
        json!({"type":"response.output_text.delta","delta":"中文结果"}),
        json!({"type":"response.completed","response":{"id":"response-a","usage":{"input_tokens":1,"output_tokens":2,"total_tokens":3}}})
    );
    for byte in input.bytes() {
        tx.send(Some(bytes::Bytes::from(vec![byte]))).unwrap();
    }
    drop(tx);
    let result = client
        .complete(&[Message::user("synthetic prompt")], None, &[])
        .await
        .unwrap();
    assert_eq!(result.text(), "中文结果");
    assert_eq!(result.usage.total_tokens, 3);
}

#[test]
fn api_responses_keep_their_own_endpoint_parameters_and_leave_account_auth_fixed() {
    let mut config = crate::llm::LlmConfig::new("openai-responses", "model-a", "synthetic-api-key")
        .with_base_url("http://127.0.0.1:9000/v1");
    config.max_tokens = Some(24);
    config.temperature = Some(0.5);
    let client = CodexLoginClient::for_api(config, Default::default(), default_http_client());
    let body = client.build_body(&[Message::user("hello")], None, &[], true);
    assert_eq!(body["max_output_tokens"], 24);
    assert_eq!(body["temperature"], 0.5);
    assert!(body.get("messages").is_none());
    assert!(body.get("max_tokens").is_none());
    assert!(body["input"].is_array());
    assert_eq!(
        client.api.as_ref().unwrap().endpoint,
        "http://127.0.0.1:9000/v1/responses"
    );
    assert!(client.account_id.is_empty());
    assert!(codex_client(Arc::new(RefusedHttp(401))).api.is_none());
}
