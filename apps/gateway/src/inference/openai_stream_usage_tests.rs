use super::*;
use crate::config::InferenceEndpoint;
use http_body_util::Full;

#[tokio::test]
async fn chat_usage_options_do_not_leak_into_responses_or_other_native_dialects() {
    let headers = HeaderMap::from_iter([(
        http::header::CONTENT_TYPE,
        http::HeaderValue::from_static("application/json"),
    )]);
    for (endpoint, expects_options) in [
        (InferenceEndpoint::ChatCompletions, true),
        (InferenceEndpoint::Completions, true),
        (InferenceEndpoint::Responses, false),
        (InferenceEndpoint::Messages, false),
        (InferenceEndpoint::Embeddings, false),
    ] {
        for streaming in [false, true] {
            let body = serde_json::to_vec(
                &serde_json::json!({"model":"m","input":"hi","stream":streaming}),
            )
            .unwrap();
            let mut request = collect_json_body(&headers, Full::new(Bytes::from(body)))
                .await
                .unwrap();
            request.request_stream_usage(endpoint);
            let body: serde_json::Value =
                serde_json::from_slice(&request.routed_body("upstream", None).unwrap()).unwrap();
            assert_eq!(
                body.get("stream_options").is_some(),
                expects_options && streaming,
                "{endpoint:?}, streaming={streaming}"
            );
            if expects_options && streaming {
                assert_eq!(body["stream_options"]["include_usage"], true);
            }
        }
    }
}

#[tokio::test]
async fn preserves_an_explicit_chat_usage_option() {
    let headers = HeaderMap::from_iter([(
        http::header::CONTENT_TYPE,
        http::HeaderValue::from_static("application/json"),
    )]);
    let mut request = collect_json_body(
        &headers,
        Full::new(Bytes::from_static(
            br#"{"model":"m","stream":true,"stream_options":{"include_usage":false}}"#,
        )),
    )
    .await
    .unwrap();
    request.request_stream_usage(InferenceEndpoint::ChatCompletions);
    let body: serde_json::Value =
        serde_json::from_slice(&request.routed_body("upstream", None).unwrap()).unwrap();
    assert_eq!(body["stream_options"]["include_usage"], false);
}
