use super::*;
use crate::inference::{track_attributed_token_budget_response, ObservedTokenUsage};
use crate::translation::ResponseTranslation;
use http_body_util::BodyExt;

#[tokio::test]
async fn translated_json_and_sse_meter_only_upstream_observations() {
    for streaming in [false, true] {
        for usage in [
            serde_json::json!({"prompt_tokens":13,"completion_tokens":8,"total_tokens":21}),
            serde_json::json!({"prompt_tokens":0,"completion_tokens":0,"total_tokens":0}),
            serde_json::json!({"total_tokens":21}),
            serde_json::json!({}),
            serde_json::Value::Null,
        ] {
            let payload = if streaming {
                format!(
                    "data: {}\n\ndata: [DONE]\n\n",
                    serde_json::json!({
                        "id":"c1","model":"m","choices":[{"index":0,"delta":{"content":"ok"},"finish_reason":"stop"}],"usage":usage,
                    })
                )
            } else {
                serde_json::json!({"id":"c1","model":"m","choices":[{"index":0,"message":{"role":"assistant","content":"ok"},"finish_reason":"stop"}],"usage":usage}).to_string()
            };
            let response = http::Response::builder()
                .header(
                    http::header::CONTENT_TYPE,
                    if streaming {
                        "text/event-stream"
                    } else {
                        "application/json"
                    },
                )
                .body(full_body(payload))
                .unwrap();
            let (response, upstream_tokens) = observe_translation_usage(
                response,
                Some(ResponseTranslation::ResponsesFromChat),
                Some(crate::config::InferenceEndpoint::Responses),
            );
            let (parts, body) = response.into_parts();
            let body = if streaming {
                ResponseBody::boxed(
                    crate::translation_stream::TranslatedStreamBody::to_responses(body),
                )
            } else {
                let bytes = body.collect().await.unwrap().to_bytes();
                let value = ResponseTranslation::ResponsesFromChat
                    .translate_json_response(&serde_json::from_slice(&bytes).unwrap(), 42)
                    .unwrap();
                full_body(serde_json::to_vec(&value).unwrap())
            };
            let sink = Arc::new(ObservedTokenUsage::new());
            let response = track_attributed_token_budget_response(
                http::Response::from_parts(parts, body),
                None,
                Some(sink.clone()),
                None,
                upstream_tokens,
            );
            response.into_body().collect().await.unwrap();
            assert_eq!(
                sink.total(),
                usage["total_tokens"].as_u64(),
                "streaming={streaming}, usage={usage}"
            );
            if let Some(lanes) = sink.breakdown() {
                assert_eq!(lanes.prompt_tokens, usage["prompt_tokens"].as_u64());
                assert_eq!(lanes.completion_tokens, usage["completion_tokens"].as_u64());
                assert_eq!(lanes.cached_input_tokens, None);
                assert_eq!(lanes.reasoning_tokens, None);
            } else {
                assert!(usage.get("prompt_tokens").is_none());
            }
        }
    }
}
