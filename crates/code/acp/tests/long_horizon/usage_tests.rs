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
