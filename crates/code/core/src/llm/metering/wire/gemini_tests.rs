use super::*;
use serde_json::json;

fn usage() -> Value {
    json!({"promptTokenCount":10,"toolUsePromptTokenCount":3,"candidatesTokenCount":6,
        "thoughtsTokenCount":4,"cachedContentTokenCount":4,"totalTokenCount":23})
}

#[test]
fn gemini_protocol_round_trips_and_defers_model_url_selection() {
    let protocol = UsageProtocol::parse("gemini").unwrap();
    assert_eq!(protocol, UsageProtocol::Gemini);
    assert_eq!(protocol.as_str(), "gemini");
    assert_eq!(serde_json::to_value(protocol).unwrap(), "gemini");
    assert_eq!(
        protocol.endpoint("https://api.example.test/proxy/v1beta?tenant=a"),
        "https://api.example.test/proxy/v1beta?tenant=a"
    );
    assert!(UsageProtocol::parse("google").is_err());
}

#[test]
fn gemini_counts_tool_input_thoughts_and_cached_prompt_once() {
    let mut meter = WireMeter::new(UsageProtocol::Gemini);
    meter
        .response(&json!({"modelVersion":"served-model","usageMetadata":usage()}))
        .unwrap();
    let measured = meter.usage().unwrap();
    assert_eq!(measured.input_tokens, Some(13));
    assert_eq!(measured.output_tokens, Some(10));
    assert_eq!(measured.cache_read_tokens, Some(4));
    assert_eq!(measured.cache_write_tokens, Some(0));
    assert_eq!(measured.no_cache_tokens, Some(9));
    assert_eq!(measured.total_tokens().unwrap(), Some(23));
    assert_eq!(meter.response_model.as_deref(), Some("served-model"));
    assert!(meter.finished && meter.usage_complete && !meter.failed);
}

#[test]
fn gemini_cache_read_cannot_exceed_the_base_prompt_even_with_tool_input() {
    for counts in [
        json!({"promptTokenCount":10,"toolUsePromptTokenCount":3,"candidatesTokenCount":6,"thoughtsTokenCount":4,"cachedContentTokenCount":11,"totalTokenCount":23}),
        json!({"toolUsePromptTokenCount":3,"candidatesTokenCount":6,"thoughtsTokenCount":4,"cachedContentTokenCount":11,"totalTokenCount":23}),
    ] {
        let mut meter = WireMeter::new(UsageProtocol::Gemini);
        assert!(meter.response(&json!({"usageMetadata":counts})).is_err());
    }
}

#[test]
fn gemini_proto_additive_defaults_do_not_turn_missing_base_counters_into_zero() {
    let mut meter = WireMeter::new(UsageProtocol::Gemini);
    meter.response(&json!({"usageMetadata":{"promptTokenCount":10,"candidatesTokenCount":6,"totalTokenCount":16}})).unwrap();
    let measured = meter.usage().unwrap();
    assert_eq!(measured.total_tokens().unwrap(), Some(16));
    assert_eq!(measured.no_cache_tokens, Some(10));
    for (counts, expected_input, expected_output) in [
        (json!({"promptTokenCount":10,"totalTokenCount":10}), 10, 0),
        (
            json!({"candidatesTokenCount":6,"thoughtsTokenCount":4,"toolUsePromptTokenCount":3,"cachedContentTokenCount":4,"totalTokenCount":23}),
            13,
            10,
        ),
        (json!({"totalTokenCount":0}), 0, 0),
    ] {
        let mut meter = WireMeter::new(UsageProtocol::Gemini);
        meter.response(&json!({"usageMetadata":counts})).unwrap();
        let measured = meter.usage().unwrap();
        assert_eq!(measured.input_tokens, Some(expected_input));
        assert_eq!(measured.output_tokens, Some(expected_output));
    }
    for value in [
        json!({}),
        json!({"usageMetadata":null}),
        json!({"usageMetadata":{}}),
        json!({"usageMetadata":{"promptTokenCount":10,"candidatesTokenCount":null}}),
        json!({"usageMetadata":{"promptTokenCount":10,"candidatesTokenCount":6,"thoughtsTokenCount":null}}),
    ] {
        let mut meter = WireMeter::new(UsageProtocol::Gemini);
        meter.response(&value).unwrap();
        assert_eq!(
            meter.usage().unwrap().total_tokens().unwrap(),
            None,
            "{value}"
        );
    }
}

#[test]
fn gemini_refuses_invalid_decreasing_overflowing_and_inconsistent_usage() {
    for key in [
        "promptTokenCount",
        "toolUsePromptTokenCount",
        "candidatesTokenCount",
        "thoughtsTokenCount",
        "cachedContentTokenCount",
        "totalTokenCount",
    ] {
        for invalid in [json!(-1), json!(1.5), json!("3")] {
            let mut counts = usage();
            counts[key] = invalid;
            let mut meter = WireMeter::new(UsageProtocol::Gemini);
            assert!(
                meter.response(&json!({"usageMetadata":counts})).is_err(),
                "{key}"
            );
            assert!(meter.failed && !meter.usage_complete);
        }
    }
    for counts in [
        json!({"promptTokenCount":10,"totalTokenCount":9}),
        json!({"candidatesTokenCount":6,"thoughtsTokenCount":4,"totalTokenCount":9}),
        json!({"promptTokenCount":10,"thoughtsTokenCount":4,"totalTokenCount":12}),
        json!({"promptTokenCount":10,"candidatesTokenCount":6,"totalTokenCount":15}),
        json!({"promptTokenCount":u64::MAX,"toolUsePromptTokenCount":1,"candidatesTokenCount":0}),
        json!({"promptTokenCount":10,"candidatesTokenCount":u64::MAX,"thoughtsTokenCount":1}),
        json!({"promptTokenCount":10,"candidatesTokenCount":1,"cachedContentTokenCount":11}),
    ] {
        assert!(WireMeter::new(UsageProtocol::Gemini)
            .response(&json!({"usageMetadata":counts}))
            .is_err());
    }
    let mut meter = WireMeter::new(UsageProtocol::Gemini);
    meter
        .event(&json!({"usageMetadata":usage()}).to_string())
        .unwrap();
    assert!(meter
        .event(&json!({"usageMetadata":{"candidatesTokenCount":5}}).to_string())
        .is_err());
    assert!(meter.failed && !meter.usage_complete);
}

#[test]
fn gemini_invalid_next_snapshot_keeps_the_last_valid_consumption() {
    let mut meter = WireMeter::new(UsageProtocol::Gemini);
    meter
        .event(&json!({"usageMetadata":usage()}).to_string())
        .unwrap();
    assert!(meter.event(&json!({"usageMetadata":{"promptTokenCount":11,"candidatesTokenCount":5,"totalTokenCount":24}}).to_string()).is_err());
    assert!(meter.failed && !meter.usage_complete);
    let measured = meter
        .usage()
        .expect("invalid later usage must not destroy earlier consumption");
    assert_eq!(measured.total_tokens().unwrap(), Some(23));
    assert_eq!(measured.input_tokens, Some(13));
    assert_eq!(measured.output_tokens, Some(10));
    for (protocol, initial, invalid, total) in [
        (
            UsageProtocol::OpenaiChat,
            json!({"prompt_tokens":10,"completion_tokens":6}),
            json!({"prompt_tokens":11,"completion_tokens":5}),
            16,
        ),
        (
            UsageProtocol::OpenaiResponses,
            json!({"input_tokens":10,"output_tokens":6}),
            json!({"input_tokens":11,"output_tokens":5}),
            16,
        ),
        (
            UsageProtocol::AnthropicMessages,
            json!({"input_tokens":10,"output_tokens":6,"cache_read_input_tokens":4,"cache_creation_input_tokens":2}),
            json!({"input_tokens":11,"output_tokens":5}),
            22,
        ),
    ] {
        let mut meter = WireMeter::new(protocol);
        meter.response(&json!({"usage":initial})).unwrap();
        let previous = meter.usage().unwrap();
        assert_eq!(previous.total_tokens().unwrap(), Some(total));
        assert!(meter.response(&json!({"usage":invalid})).is_err());
        assert!(meter.failed && !meter.usage_complete);
        assert_eq!(meter.usage().unwrap(), previous);
    }
}

#[test]
fn gemini_stream_preserves_fragmented_unicode_tools_signatures_and_trailing_usage() {
    let events = [
        json!({"modelVersion":"served-model","candidates":[{"index":0,"content":{"role":"model","parts":[{"text":"中文","thought":true,"thoughtSignature":"opaque-signature"}]}}]}),
        json!({"candidates":[{"index":0,"content":{"role":"model","parts":[{"functionCall":{"name":"lookup","args":{"q":"值"}}}]}}]}),
        json!({"candidates":[{"index":0,"finishReason":"STOP"}]}),
        json!({"usageMetadata":usage()}),
    ];
    for ending in ["\n", "\r\n", "\r"] {
        let wire = events
            .iter()
            .map(|event| format!("data: {event}{ending}{ending}"))
            .collect::<String>();
        for split in 0..=wire.len() {
            let mut meter = SseMeter::new(UsageProtocol::Gemini);
            let mut frames = meter.push(&wire.as_bytes()[..split]).unwrap();
            frames.extend(meter.push(&wire.as_bytes()[split..]).unwrap());
            assert!(!meter.meter.finished && !meter.meter.usage_complete);
            frames.extend(meter.finish().unwrap());
            assert_eq!(frames.len(), 4);
            assert!(frames[..2].iter().all(|frame| frame.has_output));
            assert!(frames[2..].iter().all(|frame| !frame.has_output));
            assert!(
                frames[0].frame.contains("中文") && frames[0].frame.contains("opaque-signature")
            );
            assert!(frames[1].frame.contains("lookup") && frames[1].frame.contains("值"));
            assert!(meter.meter.finished && meter.meter.usage_complete && !meter.meter.failed);
            assert_eq!(
                meter.meter.usage().unwrap().total_tokens().unwrap(),
                Some(23)
            );
        }
    }
}

#[test]
fn gemini_waits_for_all_seen_candidates_and_the_response_eof() {
    for invalid in [json!(-1), json!(8), json!(u64::MAX)] {
        let mut meter = WireMeter::new(UsageProtocol::Gemini);
        assert!(meter
            .event(&json!({"candidates":[{"index":invalid,"finishReason":"STOP"}]}).to_string())
            .is_err());
        assert!(meter.failed);
    }
    for complete in [false, true] {
        let mut meter = SseMeter::new(UsageProtocol::Gemini);
        for event in [
            json!({"candidates":[{"index":0,"finishReason":"STOP"}]}),
            json!({"candidates":[{"index":1,"content":{"parts":[{"text":"second"}]}}]}),
            json!({"usageMetadata":usage()}),
        ] {
            meter.push(format!("data: {event}\n\n").as_bytes()).unwrap();
            assert!(!meter.meter.finished);
        }
        if complete {
            meter
                .push(b"data: {\"candidates\":[{\"index\":1,\"finishReason\":\"MAX_TOKENS\"}]}\n\n")
                .unwrap();
        }
        meter.finish().unwrap();
        assert_eq!(meter.meter.finished, complete);
        assert_eq!(meter.meter.usage_complete, complete);
    }
}

#[test]
fn gemini_blocked_prompts_are_valid_responses_but_errors_and_lost_terminals_are_not() {
    let mut blocked = SseMeter::new(UsageProtocol::Gemini);
    blocked.push(format!("data: {}\n\n",json!({"promptFeedback":{"blockReason":"SAFETY"},"usageMetadata":{"promptTokenCount":10,"candidatesTokenCount":0,"totalTokenCount":10}})).as_bytes()).unwrap();
    blocked.finish().unwrap();
    assert!(blocked.meter.finished && blocked.meter.usage_complete && !blocked.meter.failed);
    for terminal in [false, true] {
        let mut meter = SseMeter::new(UsageProtocol::Gemini);
        meter.push(format!("data: {}\n\n", json!({"candidates":[{"content":{"parts":[{"text":"partial"}]}}],"usageMetadata":usage()})).as_bytes()).unwrap();
        if terminal {
            meter
                .push(b"data: {\"error\":{\"code\":503,\"message\":\"synthetic-secret\"}}\n\n")
                .unwrap();
        }
        meter.finish().unwrap();
        assert!(!meter.meter.finished && !meter.meter.usage_complete);
        assert_eq!(meter.meter.failed, terminal);
        assert_eq!(
            meter.meter.usage().unwrap().total_tokens().unwrap(),
            Some(23)
        );
        assert!(!format!("{meter:?}").contains("synthetic-secret"));
    }
    assert!(WireMeter::new(UsageProtocol::Gemini)
        .event("[DONE]")
        .is_err());
    let mut meter = WireMeter::new(UsageProtocol::Gemini);
    meter
        .response(&json!({"modelVersion":"synthetic-secret","usageMetadata":usage()}))
        .unwrap();
    assert!(format!("{meter:?}").contains("Gemini"));
    assert!(!format!("{meter:?}").contains("synthetic-secret"));
}
