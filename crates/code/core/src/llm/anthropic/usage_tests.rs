use super::*;
use crate::llm::http::{HttpResponse, StreamingHttpResponse};
use serde_json::{json, Value};

struct UsageHttp(Value);

#[async_trait]
impl HttpClient for UsageHttp {
    async fn post(
        &self,
        _: &str,
        _: Vec<(&str, &str)>,
        _: &Value,
        _: CancellationToken,
    ) -> Result<HttpResponse> {
        Ok(HttpResponse {
            status: 200,
            body: json!({
                "content": [{"type":"text", "text":"ok"}],
                "stop_reason": "end_turn",
                "usage": self.0,
            })
            .to_string(),
            retry_after: None,
        })
    }

    async fn post_streaming(
        &self,
        _: &str,
        _: Vec<(&str, &str)>,
        _: &Value,
        _: CancellationToken,
    ) -> Result<StreamingHttpResponse> {
        let mut usage = self.0.clone();
        usage["output_tokens"] = json!(0);
        let frames = [
            json!({"type":"message_start", "message":{"usage":usage}}),
            json!({"type":"message_delta", "delta":{"stop_reason":"end_turn"}, "usage":{"output_tokens":25}}),
            json!({"type":"message_stop"}),
        ];
        let wire = frames
            .iter()
            .map(|frame| format!("data: {frame}\n\n"))
            .collect::<String>();
        Ok(StreamingHttpResponse {
            status: 200,
            retry_after: None,
            byte_stream: Box::pin(futures::stream::iter([Ok(bytes::Bytes::from(wire))])),
            error_body: String::new(),
        })
    }
}

fn cases() -> [(Value, usize, Option<usize>, Option<usize>); 4] {
    [
        (
            json!({"input_tokens":100,"output_tokens":25}),
            100,
            None,
            None,
        ),
        (
            json!({"input_tokens":100,"output_tokens":25,"cache_read_input_tokens":0,"cache_creation_input_tokens":0}),
            100,
            Some(0),
            Some(0),
        ),
        (
            json!({"input_tokens":100,"output_tokens":25,"cache_read_input_tokens":800,"cache_creation_input_tokens":100}),
            1000,
            Some(800),
            Some(100),
        ),
        (
            json!({"input_tokens":100,"output_tokens":25,"cache_read_input_tokens":800}),
            900,
            Some(800),
            None,
        ),
    ]
}

#[tokio::test]
async fn completion_counts_cached_input_once_and_preserves_cache_presence() {
    for (wire, prompt, read, write) in cases() {
        let client = AnthropicClient::new("synthetic-key".into(), "model".into())
            .with_http_client(Arc::new(UsageHttp(wire)));
        let response = client
            .complete(&[Message::user("hello")], None, &[])
            .await
            .unwrap();
        assert_eq!(response.usage.prompt_tokens, prompt);
        assert_eq!(response.usage.completion_tokens, 25);
        assert_eq!(response.usage.total_tokens, prompt + 25);
        assert_eq!(response.usage.cache_read_tokens, read);
        assert_eq!(response.usage.cache_write_tokens, write);
    }
}

#[tokio::test]
async fn streaming_counts_cached_input_once_and_preserves_cache_presence() {
    for (wire, prompt, read, write) in cases() {
        let client = AnthropicClient::new("synthetic-key".into(), "model".into())
            .with_http_client(Arc::new(UsageHttp(wire)));
        let mut events = client
            .complete_streaming(
                &[Message::user("hello")],
                None,
                &[],
                CancellationToken::new(),
            )
            .await
            .unwrap();
        let mut done = None;
        while let Some(event) = events.recv().await {
            if let StreamEvent::Done(response) = event {
                done = Some(response);
            }
        }
        let response = done.expect("stream must retain the final usage");
        assert_eq!(response.usage.prompt_tokens, prompt);
        assert_eq!(response.usage.completion_tokens, 25);
        assert_eq!(response.usage.total_tokens, prompt + 25);
        assert_eq!(response.usage.cache_read_tokens, read);
        assert_eq!(response.usage.cache_write_tokens, write);
    }
}
