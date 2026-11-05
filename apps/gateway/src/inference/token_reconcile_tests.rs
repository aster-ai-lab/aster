use super::*;
use crate::inference::LifecycleTokenBreakdown;
use http_body_util::BodyExt;

const RESPONSES_USAGE: &str = r#"{"input_tokens":13,"input_tokens_details":{"cached_tokens":5},"output_tokens":8,"output_tokens_details":{"reasoning_tokens":3},"total_tokens":21}"#;

#[test]
fn gemini_usage_counts_tool_and_thought_tokens_without_adding_cache_twice() {
    let body = br#"{"usageMetadata":{"promptTokenCount":10,"toolUsePromptTokenCount":2,"candidatesTokenCount":6,"thoughtsTokenCount":5,"cachedContentTokenCount":4,"totalTokenCount":23}}"#;
    assert_eq!(
        observe_usage_from_bytes(body),
        Some(ObservedUsage {
            prompt: Some(12),
            completion: Some(11),
            cached_input: Some(4),
            reasoning: Some(5),
            total: Some(23)
        })
    );
    for json in [
        r#"{"promptTokenCount":10,"candidatesTokenCount":6,"totalTokenCount":15}"#,
        r#"{"promptTokenCount":18446744073709551615,"toolUsePromptTokenCount":1,"candidatesTokenCount":0,"totalTokenCount":0}"#,
        r#"{"promptTokenCount":10,"candidatesTokenCount":-1}"#,
        r#"{"promptTokenCount":10,"candidatesTokenCount":6,"cachedContentTokenCount":11}"#,
        r#"{"promptTokenCount":10,"toolUsePromptTokenCount":2,"candidatesTokenCount":6,"cachedContentTokenCount":11,"totalTokenCount":18}"#,
        r#"{"promptTokenCount":10,"toolUsePromptTokenCount":null,"candidatesTokenCount":0,"totalTokenCount":5}"#,
        r#"{"promptTokenCount":0,"candidatesTokenCount":10,"thoughtsTokenCount":null,"totalTokenCount":5}"#,
        "null",
    ] {
        assert!(
            gemini_usage(&serde_json::from_str(json).unwrap()).is_none(),
            "{json}"
        );
    }
    let missing = gemini_usage(&serde_json::json!({"promptTokenCount":10})).unwrap();
    assert_eq!(missing.prompt, Some(10));
    assert_eq!(missing.completion, None);
    assert_eq!(missing.total, None);
    let blocked =
        gemini_usage(&serde_json::json!({"promptTokenCount":10,"totalTokenCount":10})).unwrap();
    assert_eq!(blocked.completion, Some(0));
}

#[test]
fn gemini_usage_survives_every_sse_split_and_a_malformed_later_snapshot() {
    let stream = concat!(
        "data: {\"candidates\":[{\"content\":{\"parts\":[{\"text\":\"中\"}]}}]}\r\n\r\n",
        "data: {\"usageMetadata\":{\"promptTokenCount\":10,\"candidatesTokenCount\":6,\"totalTokenCount\":16}}\r\n\r\n",
        "data: {\"usageMetadata\":{\"promptTokenCount\":11,\"candidatesTokenCount\":5,\"totalTokenCount\":16}}\r\n\r\n"
    );
    for split in 0..=stream.len() {
        let mut body = empty_body();
        body.sse_response = true;
        body.note_data(&stream.as_bytes()[..split]);
        body.note_data(&stream.as_bytes()[split..]);
        body.finish();
        assert_eq!(
            body.observed_sse_usage().unwrap().prompt,
            Some(10),
            "split {split}"
        );
        assert_eq!(
            body.observed_sse_usage().unwrap().completion,
            Some(6),
            "split {split}"
        );
    }
}

#[test]
fn gemini_large_response_tail_keeps_native_usage_metadata() {
    let mut body = vec![b'x'; MAX_JSON_PREFIX_BYTES + 1];
    body.extend_from_slice(br#"],"usageMetadata":{"promptTokenCount":10,"candidatesTokenCount":6,"totalTokenCount":16}}"#);
    assert_eq!(
        observe_json_usage_from_tail(&body[body.len() - MAX_JSON_TAIL_BYTES..])
            .unwrap()
            .total,
        Some(16)
    );
}

#[test]
fn gemini_partial_stream_usage_keeps_prior_input_and_cache_counters() {
    let mut body = empty_body();
    body.note_sse_event(br#"{"usageMetadata":{"promptTokenCount":10,"candidatesTokenCount":1,"cachedContentTokenCount":4,"totalTokenCount":11}}"#);
    body.note_sse_event(br#"{"usageMetadata":{"candidatesTokenCount":6,"totalTokenCount":16}}"#);
    body.note_sse_event(br#"{"usageMetadata":{}}"#);
    let usage = body.observed_sse_usage().unwrap();
    assert_eq!(usage.prompt, Some(10));
    assert_eq!(usage.completion, Some(6));
    assert_eq!(usage.cached_input, Some(4));
    assert_eq!(usage.total, Some(16));
}

fn responses_usage() -> ObservedUsage {
    ObservedUsage {
        prompt: Some(13),
        completion: Some(8),
        cached_input: Some(5),
        reasoning: Some(3),
        total: Some(21),
    }
}

#[test]
fn parses_responses_json_usage_without_losing_split_or_details() {
    let body = format!(r#"{{"object":"response","usage":{RESPONSES_USAGE}}}"#);
    assert_eq!(
        observe_usage_from_bytes(body.as_bytes()),
        Some(responses_usage())
    );
}

#[test]
fn observes_responses_terminal_sse_usage_at_every_transport_split() {
    for terminal in [
        "response.completed",
        "response.incomplete",
        "response.failed",
    ] {
        let stream = format!(
            "event: {terminal}\ndata: {{\"type\":\"{terminal}\",\"response\":{{\"usage\":{RESPONSES_USAGE}}}}}\n\n"
        );
        for split in 0..=stream.len() {
            let sink = Arc::new(ObservedTokenUsage::new());
            let mut body = empty_body();
            body.sse_response = true;
            body.observed_tokens = Some(sink.clone());
            body.note_data(&stream.as_bytes()[..split]);
            body.note_data(&stream.as_bytes()[split..]);
            body.finish();
            assert_eq!(
                body.observed_sse_usage(),
                Some(responses_usage()),
                "{terminal}, split {split}"
            );
            assert_eq!(sink.total(), Some(21), "{terminal}, split {split}");
            assert_eq!(sink.breakdown().unwrap().reasoning_tokens, Some(3));
        }
    }
}

#[test]
fn parses_responses_usage_without_inventing_missing_details() {
    let body = br#"{"object":"response","usage":{"input_tokens":13,"output_tokens":8}}"#;
    assert_eq!(
        observe_usage_from_bytes(body),
        Some(ObservedUsage {
            prompt: Some(13),
            completion: Some(8),
            total: Some(21),
            ..Default::default()
        })
    );
}

#[tokio::test]
async fn unfinished_responses_sse_does_not_settle_from_provisional_usage() {
    let response = Response::builder()
        .header(http::header::CONTENT_TYPE, "text/event-stream; charset=utf-8")
        .body(ResponseBody::full(Bytes::from_static(
            b"event: response.created\ndata: {\"type\":\"response.created\",\"response\":{\"usage\":{\"input_tokens\":0,\"output_tokens\":0,\"total_tokens\":0}}}\n\n",
        )))
        .unwrap();
    let sink = Arc::new(ObservedTokenUsage::new());
    let response = track_token_budget_response(response, None, Some(sink.clone()));
    response.into_body().collect().await.unwrap();
    assert_eq!(sink.total(), None);
    assert_eq!(sink.breakdown(), None);
}

#[tokio::test]
async fn large_multiline_responses_terminal_usage_survives_transport_chunks() {
    let output = serde_json::json!([{"type":"message","role":"assistant","content":[{"type":"output_text","text":"文".repeat(100_000)}]}]);
    for newline in ["\n", "\r\n", "\r"] {
        let stream = format!("event: response.completed\ndata: {{\"type\":\"response.completed\",\"response\":{{\"output\":{output},\ndata: \"usage\":{RESPONSES_USAGE}}}}}\n\n").replace('\n', newline);
        let frames = stream
            .as_bytes()
            .chunks(4096)
            .map(|chunk| Ok::<_, std::io::Error>(Frame::data(Bytes::copy_from_slice(chunk))))
            .collect::<Vec<_>>();
        let response = Response::builder()
            .header(http::header::CONTENT_TYPE, "text/event-stream")
            .body(ResponseBody::boxed(http_body_util::StreamBody::new(
                futures_util::stream::iter(frames),
            )))
            .unwrap();
        let sink = Arc::new(ObservedTokenUsage::new());
        let response = track_token_budget_response(response, None, Some(sink.clone()));
        let bytes = response.into_body().collect().await.unwrap().to_bytes();
        assert_eq!(bytes, stream.as_bytes(), "observer changed the wire");
        assert_eq!(sink.total(), Some(21), "newline {newline:?}");
        assert_eq!(sink.breakdown().unwrap().cached_input_tokens, Some(5));
        assert_eq!(sink.breakdown().unwrap().reasoning_tokens, Some(3));
    }
}

fn empty_body() -> TokenBudgetBody {
    TokenBudgetBody {
        inner: Box::pin(ResponseBody::full(Bytes::new())),
        admission: None,
        observed_tokens: None,
        attribution: None,
        upstream_tokens: None,
        status: 200,
        sse_response: false,
        json_prefix: Vec::new(),
        json_truncated: false,
        json_tail: Vec::new(),
        sse_decoder: Some(SseDecoder::default()),
        sse_usage: None,
        sse_anthropic: None,
        sse_gemini: serde_json::Map::new(),
    }
}

fn observe_usage_from_bytes(bytes: &[u8]) -> Option<ObservedUsage> {
    let mut body = empty_body();
    body.note_data(bytes);
    body.finish();
    body.observed_sse_usage()
        .or_else(|| body.observed_json_usage())
}

fn observe_sse_usage_from_bytes(bytes: &[u8]) -> Option<ObservedUsage> {
    let mut body = empty_body();
    body.sse_response = true;
    body.note_data(bytes);
    body.finish();
    body.observed_sse_usage()
}

fn observe_total_tokens_from_bytes(bytes: &[u8]) -> Option<u64> {
    observe_usage_from_bytes(bytes).and_then(|usage| usage.resolved_total())
}

#[test]
fn parses_anthropic_json_usage() {
    let body = br#"{"type":"message","usage":{"input_tokens":10,"cache_read_input_tokens":4,"output_tokens":6}}"#;
    assert_eq!(
        observe_usage_from_bytes(body),
        Some(ObservedUsage {
            prompt: Some(14),
            completion: Some(6),
            cached_input: Some(4),
            reasoning: None,
            total: Some(20),
        })
    );
}

#[test]
fn parses_anthropic_sse_usage_across_start_and_delta() {
    let body = b"event: message_start\ndata: {\"type\":\"message_start\",\"message\":{\"usage\":{\"input_tokens\":25,\"output_tokens\":1}}}\n\n\
event: content_block_delta\ndata: {\"type\":\"content_block_delta\",\"delta\":{\"text\":\"hi\"}}\n\n\
event: message_delta\ndata: {\"type\":\"message_delta\",\"usage\":{\"output_tokens\":15}}\n\n";
    assert_eq!(
        observe_sse_usage_from_bytes(body),
        Some(ObservedUsage {
            prompt: Some(25),
            completion: Some(15),
            cached_input: None,
            reasoning: None,
            total: Some(40),
        })
    );
}

#[test]
fn parses_openai_cache_and_reasoning_details() {
    let body = br#"{"id":"1","usage":{"prompt_tokens":100,"prompt_tokens_details":{"cached_tokens":60},"completion_tokens":7,"completion_tokens_details":{"reasoning_tokens":5},"total_tokens":107}}"#;
    assert_eq!(
        observe_usage_from_bytes(body),
        Some(ObservedUsage {
            prompt: Some(100),
            completion: Some(7),
            cached_input: Some(60),
            reasoning: Some(5),
            total: Some(107),
        })
    );
}

#[test]
fn total_only_usage_leaves_split_lanes_unknown() {
    let body = br#"{"usage":{"total_tokens":42}}"#;
    assert_eq!(
        observe_usage_from_bytes(body),
        Some(ObservedUsage {
            prompt: None,
            completion: None,
            cached_input: None,
            reasoning: None,
            total: Some(42),
        })
    );
}

#[test]
fn parses_json_usage_total_tokens() {
    let body = br#"{"id":"1","usage":{"prompt_tokens":3,"completion_tokens":7,"total_tokens":10}}"#;
    assert_eq!(observe_total_tokens_from_bytes(body), Some(10));
}

#[test]
fn parses_json_usage_without_total_field() {
    let body = br#"{"usage":{"prompt_tokens":2,"completion_tokens":5}}"#;
    assert_eq!(observe_total_tokens_from_bytes(body), Some(7));
}

#[test]
fn parses_sse_usage_from_last_data_line() {
    let body =
        b"data: {\"choices\":[]}\n\ndata: {\"usage\":{\"total_tokens\":42}}\n\ndata: [DONE]\n\n";
    assert_eq!(
        observe_sse_usage_from_bytes(body).unwrap().resolved_total(),
        Some(42)
    );
}

#[test]
fn observes_sse_usage_after_many_events_without_json_buffering() {
    let sink = std::sync::Arc::new(ObservedTokenUsage::new());
    let mut body = empty_body();
    body.sse_response = true;
    body.observed_tokens = Some(sink.clone());

    let padding_line = b"data: {\"choices\":[{\"delta\":{\"content\":\"x\"}}]}\n\n";
    let mut padded = 0usize;
    while padded <= MAX_JSON_PREFIX_BYTES {
        body.note_data(padding_line);
        padded += padding_line.len();
    }
    assert!(body.json_prefix.is_empty());
    assert!(body.json_tail.is_empty());
    body.note_data(b"data: {\"usage\":{\"total_tokens\":99}}\n\ndata: [DONE]\n\n");
    body.finish();
    assert_eq!(sink.total(), Some(99));
}

#[test]
fn observes_sse_usage_split_across_frames() {
    let sink = std::sync::Arc::new(ObservedTokenUsage::new());
    let mut body = empty_body();
    body.sse_response = true;
    body.observed_tokens = Some(sink.clone());
    body.note_data(b"data: {\"usage\":{\"total_tok");
    body.note_data(b"ens\":17}}\n\ndata: [DONE]\n\n");
    body.finish();
    assert_eq!(sink.total(), Some(17));
}

#[test]
fn observes_json_usage_after_body_exceeds_prefix_budget() {
    let sink = std::sync::Arc::new(ObservedTokenUsage::new());
    let mut body = empty_body();
    body.observed_tokens = Some(sink.clone());

    body.note_data(b"{\"id\":\"1\",\"choices\":[{\"message\":{\"content\":\"");
    let mut padded = body.json_prefix.len();
    let chunk = b"x".repeat(4096);
    while padded <= MAX_JSON_PREFIX_BYTES {
        body.note_data(&chunk);
        padded += chunk.len();
    }
    assert!(body.json_truncated);
    body.note_data(
        b"\"}}],\"usage\":{\"prompt_tokens\":2,\"completion_tokens\":3,\"total_tokens\":88}}",
    );
    body.finish();
    assert_eq!(sink.total(), Some(88));
}

#[test]
fn sink_records_the_full_token_split() {
    let sink = std::sync::Arc::new(ObservedTokenUsage::new());
    let mut body = empty_body();
    body.sse_response = true;
    body.observed_tokens = Some(sink.clone());
    body.note_data(
        b"data: {\"usage\":{\"prompt_tokens\":9,\"completion_tokens\":4,\"total_tokens\":13}}\n\n",
    );
    body.finish();
    assert_eq!(sink.total(), Some(13));
    assert_eq!(
        sink.breakdown(),
        Some(LifecycleTokenBreakdown {
            prompt_tokens: Some(9),
            completion_tokens: Some(4),
            cached_input_tokens: None,
            reasoning_tokens: None,
        })
    );
}
