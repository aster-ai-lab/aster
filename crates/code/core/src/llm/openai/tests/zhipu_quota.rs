use crate::llm::http::{HttpClient, HttpResponse, StreamingHttpResponse};
use crate::llm::{zhipu::ZhipuClient, LlmClient, Message, NonRetryableLlmError, OpenAiClient};
use crate::retry::{RetryConfig, RetryExhaustedError};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use tokio_util::sync::CancellationToken;

struct ErrorHttp {
    calls: AtomicUsize,
    status: u16,
    body: String,
}

#[async_trait::async_trait]
impl HttpClient for ErrorHttp {
    async fn post(
        &self,
        _url: &str,
        _headers: Vec<(&str, &str)>,
        _body: &serde_json::Value,
        _cancel: CancellationToken,
    ) -> anyhow::Result<HttpResponse> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        Ok(HttpResponse {
            status: self.status,
            body: self.body.clone(),
            retry_after: Some("30".into()),
        })
    }

    async fn post_streaming(
        &self,
        _url: &str,
        _headers: Vec<(&str, &str)>,
        _body: &serde_json::Value,
        _cancel: CancellationToken,
    ) -> anyhow::Result<StreamingHttpResponse> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        Ok(StreamingHttpResponse {
            status: self.status,
            retry_after: Some("30".into()),
            byte_stream: Box::pin(futures::stream::empty()),
            error_body: self.body.clone(),
        })
    }
}

async fn invoke(
    provider: &str,
    status: u16,
    body: &str,
    streaming: bool,
) -> (anyhow::Error, usize) {
    let http = Arc::new(ErrorHttp {
        calls: AtomicUsize::new(0),
        status,
        body: body.into(),
    });
    // A GLM model name alone must not grant another provider GLM error semantics.
    let retry = RetryConfig {
        max_retries: 2,
        base_delay_ms: 1,
        max_delay_ms: 1,
        ..RetryConfig::default()
    };
    let client: Arc<dyn LlmClient> = if provider == "zhipu" {
        Arc::new(
            ZhipuClient::new("synthetic-test-key".into(), "glm-5.3".into())
                .with_http_client(http.clone())
                .with_retry_config(retry),
        )
    } else {
        Arc::new(
            OpenAiClient::new("synthetic-test-key".into(), "glm-5.3".into())
                .with_provider_name(provider)
                .with_http_client(http.clone())
                .with_retry_config(retry),
        )
    };
    let messages = [Message::user("synthetic request")];
    let error = if streaming {
        client
            .complete_streaming(&messages, None, &[], CancellationToken::new())
            .await
            .expect_err("the controlled response is an HTTP error")
    } else {
        client
            .complete(&messages, None, &[])
            .await
            .expect_err("the controlled response is an HTTP error")
    };
    let calls = http.calls.load(Ordering::SeqCst);
    eprintln!("zhipu quota fixture: provider={provider} status={status} streaming={streaming} calls={calls}");
    (error, calls)
}

#[tokio::test(start_paused = true)]
async fn zhipu_1113_stops_both_transports_without_waiting() {
    let body = r#"{"error":{"code":"1113","message":"opaque provider diagnostic"}}"#;
    for streaming in [false, true] {
        let started = tokio::time::Instant::now();
        let (error, calls) = invoke("zhipu", 429, body, streaming).await;
        assert_eq!(calls, 1, "streaming={streaming}: {error}");
        assert_eq!(started.elapsed(), std::time::Duration::ZERO);
        let typed = error
            .downcast_ref::<NonRetryableLlmError>()
            .expect("quota failures must remain terminal at the outer Agent boundary");
        assert_eq!(typed.provider(), Some("zhipu"));
        assert_eq!(typed.status(), Some(429));
        assert!(typed.to_string().contains("1113"));
        assert!(crate::llm::non_retryable_llm_error_message(&error).is_some());
    }
}

#[tokio::test(start_paused = true)]
async fn zhipu_transient_unknown_and_malformed_429_keep_existing_retries() {
    for body in [
        r#"{"error":{"code":"1302","message":"rate limited"}}"#,
        r#"{"error":{"code":"1305","message":"busy"}}"#,
        r#"{"error":{"code":"unknown","message":"opaque"}}"#,
        r#"{"error":{"code":"11130","message":"opaque"}}"#,
        r#"{"error":{"code":1113,"message":"opaque"}}"#,
        r#"{"error":{"message":"1113 insufficient balance"}}"#,
        r#"{"code":"1113"}"#,
        "1113 insufficient balance",
        "{",
    ] {
        for streaming in [false, true] {
            let started = tokio::time::Instant::now();
            let (error, calls) = invoke("zhipu", 429, body, streaming).await;
            assert_eq!(calls, 3, "streaming={streaming} body={body}: {error}");
            assert!(started.elapsed() >= std::time::Duration::from_secs(60));
            let typed = error
                .downcast_ref::<RetryExhaustedError>()
                .expect("unclassified 429 must retain the existing retry exhaustion");
            assert_eq!(typed.attempts(), 3);
            assert_eq!(typed.status().as_u16(), 429);
        }
    }
}

#[tokio::test(start_paused = true)]
async fn zhipu_code_does_not_change_other_providers_or_other_statuses() {
    let body = r#"{"error":{"code":"1113","message":"opaque"}}"#;
    for (provider, status, expected_calls) in [
        ("openai", 429, 3),
        ("deepseek", 429, 3),
        ("custom-zhipu", 429, 3),
        ("zhipu", 503, 3),
        ("zhipu", 400, 1),
    ] {
        for streaming in [false, true] {
            let (error, calls) = invoke(provider, status, body, streaming).await;
            assert_eq!(
                calls, expected_calls,
                "{provider} streaming={streaming}: {error}"
            );
            if expected_calls == 3 {
                assert!(error.downcast_ref::<RetryExhaustedError>().is_some());
            } else {
                assert_eq!(
                    error
                        .downcast_ref::<NonRetryableLlmError>()
                        .unwrap()
                        .status(),
                    Some(400)
                );
            }
        }
    }
}
