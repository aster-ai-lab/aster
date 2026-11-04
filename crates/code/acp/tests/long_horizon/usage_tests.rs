#![cfg(test)]

use super::*;
use serde_json::json;

fn valid() -> serde_json::Value {
    json!({"prompt_tokens":100,"completion_tokens":7,"total_tokens":107,"prompt_tokens_details":{"cached_tokens":80},"prompt_cache_hit_tokens":80,"prompt_cache_miss_tokens":20,"completion_tokens_details":{"reasoning_tokens":3}})
}

#[test]
fn raw_usage_validates_numbers_aliases_and_never_invents_absent_counters() {
    let observed = Observation::json(
        200,
        &serde_json::to_vec(
            &json!({"usage":valid(),"choices":[{"reasoning_content":"private-thought"}]}),
        )
        .unwrap(),
    );
    assert_eq!(observed.state, State::Present);
    let usage = observed.usage.as_ref().unwrap();
    assert_eq!(usage.total_tokens, 107);
    assert_eq!(usage.cache_read_tokens, Some(80));
    assert_eq!(usage.cache_miss_tokens, Some(20));
    assert_eq!(usage.reasoning_tokens, Some(3));
    assert!(!serde_json::to_string(&observed)
        .unwrap()
        .contains("private-"));
    for body in [b"{}".as_slice(), b"{\"usage\":null}"] {
        let missing = Observation::json(200, body);
        assert_eq!(missing.state, State::Missing);
        assert!(missing.usage.is_none());
    }
    let zero = Observation::json(
        200,
        br#"{"usage":{"prompt_tokens":0,"completion_tokens":0,"total_tokens":0}}"#,
    );
    assert_eq!(zero.state, State::Present);
    assert_eq!(zero.usage.unwrap().cache_read_tokens, None);
    for name in ["incomplete", "missing", "invalid", "present"] {
        assert_eq!(State::parse(name).unwrap().as_str(), name);
    }
    assert!(State::parse("private-unknown").is_err());
}

#[test]
fn malformed_or_contradictory_usage_never_becomes_a_known_zero() {
    for (field, value) in [
        ("prompt_tokens", json!(-1)),
        ("completion_tokens", json!("7")),
        ("total_tokens", json!(108)),
        ("prompt_cache_hit_tokens", json!(79)),
        ("prompt_cache_miss_tokens", json!(21)),
        ("prompt_tokens_details", json!({"cached_tokens":101})),
        ("completion_tokens_details", json!({"reasoning_tokens":8})),
        ("prompt_tokens_details", json!([80])),
    ] {
        let mut usage = valid();
        usage[field] = value;
        let observed =
            Observation::json(200, &serde_json::to_vec(&json!({"usage":usage})).unwrap());
        assert_eq!(observed.state, State::Invalid, "{field}");
        assert!(observed.usage.is_none());
    }
    for payload in [b"{".as_slice(), b"[]", b"[null]", b"{\"usage\":[]}", b"{\"usage\":[100,7,107]}", b"{\"usage\":{}}", br#"{"usage":{"prompt_tokens":100,"prompt_tokens":100,"completion_tokens":7,"total_tokens":107}}"#, br#"{"usage":{"prompt_tokens":18446744073709551615,"completion_tokens":1,"total_tokens":0}}"#] {
        assert_eq!(Observation::json(200, payload).state, State::Invalid);
    }
}

#[test]
fn sse_usage_survives_every_byte_split_multiline_data_and_crlf() {
    let payload = format!(": comment\r\ndata: {{\"choices\":[{{\"delta\":{{\"content\":\"中文\",\"reasoning_content\":\"private-thought\"}}}}]}}\r\n\r\ndata: {{\"usage\":\r\ndata: {}}}\r\n\r\ndata: [DONE]\r\n\r\n", valid());
    for split in 0..=payload.len() {
        let mut sse = Sse::default();
        sse.feed(&payload.as_bytes()[..split]);
        sse.feed(&payload.as_bytes()[split..]);
        let observed = sse.finish(200);
        assert_eq!(observed.state, State::Present, "split {split}");
        assert_eq!(observed.usage.unwrap().total_tokens, 107);
    }
    let mut sse = Sse::default();
    for byte in payload.bytes() {
        sse.feed(&[byte]);
    }
    assert_eq!(sse.finish(200).state, State::Present);
}

#[test]
fn incomplete_oversized_duplicate_and_conflicting_streams_keep_honest_usage() {
    let frame = format!("data: {{\"usage\":{}}}\n\n", valid());
    for payload in [
        "".to_owned(),
        frame.clone(),
        format!("{frame}data: [DONE]\n"),
    ] {
        let mut sse = Sse::default();
        sse.feed(payload.as_bytes());
        let observed = sse.finish(200);
        assert_eq!(observed.state, State::Incomplete);
        assert!(observed.usage.is_none());
    }
    let mut sse = Sse::default();
    sse.feed(format!("{frame}{frame}data: [DONE]\n\n").as_bytes());
    assert_eq!(
        sse.finish(200).usage.unwrap().total_tokens,
        107,
        "identical snapshots do not add"
    );
    for payload in [format!("{frame}data: {{\"usage\":{{\"prompt_tokens\":2,\"completion_tokens\":1,\"total_tokens\":3}}}}\n\ndata: [DONE]\n\n"), format!("data: [DONE]\n\n{frame}"), "data: {broken}\n\ndata: [DONE]\n\n".into(), format!("data: {}\n\ndata: [DONE]\n\n", "x".repeat(EVENT_LIMIT+1))] {
        let mut sse = Sse::default(); sse.feed(payload.as_bytes());
        assert!(sse.line.len() <= EVENT_LIMIT && sse.event.len() <= EVENT_LIMIT);
        let observed = sse.finish(200);
        assert_eq!(observed.state, State::Invalid);
        assert!(observed.usage.is_none());
    }
}

#[test]
fn native_messages_usage_counts_cache_input_without_retaining_response_text() {
    let payload = json!({"type":"message","stop_reason":"max_tokens","content":[{"type":"thinking","thinking":"private-native-thought"}],"usage":{"input_tokens":20,"output_tokens":7,"cache_read_input_tokens":80,"cache_creation_input_tokens":10}});
    let observed = Observation::native_json(
        super::super::Protocol::Anthropic,
        200,
        &serde_json::to_vec(&payload).unwrap(),
    );
    assert_eq!(observed.state, State::Present);
    let counts = observed.usage.as_ref().unwrap();
    assert_eq!(counts.prompt_tokens, 110);
    assert_eq!(counts.completion_tokens, 7);
    assert_eq!(counts.total_tokens, 117);
    assert_eq!(counts.cache_read_tokens, Some(80));
    assert!(!serde_json::to_string(&observed)
        .unwrap()
        .contains("private-native"));
}

fn messages_stream() -> String {
    [
        json!({"type":"message_start","message":{"usage":{"input_tokens":20,"output_tokens":1,"cache_read_input_tokens":80,"cache_creation_input_tokens":10}}}),
        json!({"type":"content_block_delta","delta":{"type":"thinking_delta","thinking":"private-native-thought"}}),
        json!({"type":"message_delta","delta":{"stop_reason":null},"usage":{"output_tokens":5}}),
        json!({"type":"message_delta","delta":{"stop_reason":"max_tokens"},"usage":{"output_tokens":7}}),
        json!({"type":"message_stop"}),
    ].into_iter().map(|frame| format!("data: {frame}\r\n\r\n")).collect()
}

#[test]
fn native_messages_stream_is_cumulative_fragment_safe_and_keeps_stop_reason() {
    let payload = messages_stream();
    for split in 0..=payload.len() {
        let mut reader = Sse::for_protocol(super::super::Protocol::Anthropic);
        reader.feed(&payload.as_bytes()[..split]);
        reader.feed(&payload.as_bytes()[split..]);
        let observed = reader.finish(200);
        assert_eq!(observed.state, State::Present, "split {split}");
        assert_eq!(observed.stop_reason.as_deref(), Some("max_tokens"));
        let usage = observed.usage.as_ref().unwrap();
        assert_eq!(usage.total_tokens, 117);
        assert_eq!(
            usage.completion_tokens, 7,
            "snapshots are not added together"
        );
        assert_eq!(usage.cache_write_tokens, Some(10));
        assert_eq!(usage.cache_miss_tokens, None);
        assert_eq!(usage.reasoning_tokens, None);
        assert!(!serde_json::to_string(&observed)
            .unwrap()
            .contains("private-native"));
    }
}

#[test]
fn native_messages_missing_interrupted_invalid_and_zero_usage_stay_distinct() {
    let protocol = super::super::Protocol::Anthropic;
    assert_eq!(
        Observation::native_json(protocol, 200, b"{}").state,
        State::Missing
    );
    for bytes in [
        br#"{"usage":[20,1,80,10]}"#.as_slice(),
        br#"{"usage":{"input_tokens":20,"input_tokens":20,"output_tokens":1}}"#,
        br#"{"usage":{"input_tokens":-1,"output_tokens":1}}"#,
        br#"{"usage":{"input_tokens":"20","output_tokens":1}}"#,
        br#"{"usage":{"input_tokens":18446744073709551615,"output_tokens":1}}"#,
        br#"{"usage":{"input_tokens":18446744073709551615,"output_tokens":0,"cache_read_input_tokens":1}}"#,
    ] {
        let observed = Observation::native_json(protocol,200,bytes);
        assert_eq!(observed.state, State::Invalid);
        assert!(observed.usage.is_none());
    }
    let payload = messages_stream();
    let cut = payload.rfind("data:").unwrap();
    let mut reader = Sse::for_protocol(protocol);
    reader.feed(&payload.as_bytes()[..cut]);
    let observed = reader.finish(200);
    assert_eq!(observed.state, State::Incomplete);
    assert!(observed.usage.is_none());
    assert!(observed.stop_reason.is_none());
    for payload in [
        format!("{payload}data: {{\"type\":\"message_stop\"}}\n\n"),
        payload.replace("\"output_tokens\":7", "\"output_tokens\":4"),
        payload.replace("message_stop", "unrecognized_terminal"),
        format!("data: {}\n\n", "x".repeat(EVENT_LIMIT + 1)),
    ] {
        let mut reader = Sse::for_protocol(protocol);
        reader.feed(payload.as_bytes());
        let observed = reader.finish(200);
        assert_eq!(observed.state, State::Invalid);
        assert!(observed.usage.is_none());
    }
    let zero = br#"{"usage":{"input_tokens":0,"output_tokens":0},"stop_reason":"end_turn"}"#;
    let observed = Observation::native_json(protocol, 200, zero);
    assert_eq!(observed.state, State::Present);
    assert_eq!(observed.usage.unwrap().total_tokens, 0);
}
