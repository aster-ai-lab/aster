use super::super::http::{HttpResponse, StreamingHttpResponse};
use super::*;

struct CachedResponse;

#[async_trait]
impl HttpClient for CachedResponse {
    async fn post(
        &self,
        _: &str,
        _: Vec<(&str, &str)>,
        _: &serde_json::Value,
        _: CancellationToken,
    ) -> Result<HttpResponse> {
        Ok(HttpResponse { status:200,retry_after:None,body:r#"{"content":[{"type":"text","text":"ok"}],"stop_reason":"end_turn","usage":{"input_tokens":20,"output_tokens":2,"cache_read_input_tokens":8,"cache_creation_input_tokens":2}}"#.into() })
    }
    async fn post_streaming(
        &self,
        _: &str,
        _: Vec<(&str, &str)>,
        _: &serde_json::Value,
        _: CancellationToken,
    ) -> Result<StreamingHttpResponse> {
        anyhow::bail!("this fixture exercises the real non-streaming client")
    }
}

#[tokio::test]
async fn non_streaming_messages_normalizes_disjoint_input_buckets_once() {
    let client = AnthropicClient::new("synthetic-key".into(), "fixture".into())
        .with_http_client(Arc::new(CachedResponse));
    let response = client
        .complete(&[Message::user("fixture")], None, &[])
        .await
        .unwrap();
    assert_eq!(response.text(), "ok");
    assert_eq!(
        (
            response.usage.prompt_tokens,
            response.usage.completion_tokens,
            response.usage.total_tokens
        ),
        (30, 2, 32)
    );
    assert_eq!(
        (
            response.usage.cache_read_tokens,
            response.usage.cache_write_tokens
        ),
        (Some(8), Some(2))
    );
}
