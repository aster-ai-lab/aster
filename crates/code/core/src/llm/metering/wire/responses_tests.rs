#![cfg(test)]

use super::*;
use serde_json::json;

fn usage() -> Value {
    json!({"input_tokens":10,"output_tokens":6,"input_tokens_details":{"cached_tokens":8,"cache_write_tokens":1},"output_tokens_details":{"reasoning_tokens":4},"total_tokens":16})
}

#[test]
fn responses_protocol_and_endpoints_round_trip_without_duplicate_paths() {
    let protocol = UsageProtocol::parse("openai_responses").unwrap();
    assert_eq!(protocol, UsageProtocol::OpenaiResponses);
    assert_eq!(protocol.as_str(), "openai_responses");
    for base in [
        "https://api.example.test",
        "https://api.example.test/v1",
        "https://api.example.test/v1/responses",
        "https://api.example.test/v1/chat/completions",
    ] {
        assert_eq!(
            protocol.endpoint(base),
            "https://api.example.test/v1/responses"
        );
    }
    assert_eq!(
        protocol.endpoint("https://api.example.test/custom/v4/"),
        "https://api.example.test/custom/v4/responses"
    );
}

#[test]
fn responses_use_reported_totals_and_cache_partitions_without_adding_reasoning_twice() {
    let mut meter = WireMeter::new(UsageProtocol::OpenaiResponses);
    meter.response(&json!({"object":"response","status":"completed","model":"served-model","output":[],"usage":usage()})).unwrap();
    let measured = meter.usage().unwrap();
    assert_eq!(measured.input_tokens, Some(10));
    assert_eq!(measured.output_tokens, Some(6));
    assert_eq!(measured.cache_read_tokens, Some(8));
    assert_eq!(measured.cache_write_tokens, Some(1));
    assert_eq!(measured.no_cache_tokens, Some(1));
    assert_eq!(measured.total_tokens().unwrap(), Some(16));
    assert!(meter.usage_complete);
    assert_eq!(meter.response_model.as_deref(), Some("served-model"));
    let mut absent = WireMeter::new(UsageProtocol::OpenaiResponses);
    absent.response(&json!({"usage":null})).unwrap();
    assert_eq!(absent.usage().unwrap().total_tokens().unwrap(), None);
    assert!(!absent.usage_complete);
    let mut partial = WireMeter::new(UsageProtocol::OpenaiResponses);
    partial
        .response(&json!({"usage":{"input_tokens":0,"output_tokens":0}}))
        .unwrap();
    assert_eq!(partial.usage().unwrap().total_tokens().unwrap(), Some(0));
    assert_eq!(partial.usage().unwrap().cache_read_tokens, None);
}

#[test]
fn responses_only_finish_on_terminal_events_and_keep_failed_or_incomplete_consumption() {
    for terminal in [
        "response.completed",
        "response.incomplete",
        "response.failed",
    ] {
        let mut meter = WireMeter::new(UsageProtocol::OpenaiResponses);
        meter.event(&json!({"type":"response.created","response":{"model":"served-model","usage":null}}).to_string()).unwrap();
        assert!(!meter.finished);
        assert!(!meter.usage_complete);
        assert!(meter.event(&json!({"type":"response.output_item.added","item":{"type":"function_call","id":"call-1"}}).to_string()).unwrap());
        assert!(meter
            .event(
                &json!({"type":"response.function_call_arguments.delta","delta":"{\"name\":\"值"})
                    .to_string()
            )
            .unwrap());
        meter.event(&json!({"type":terminal,"response":{"model":"served-model","output":[],"usage":usage()}}).to_string()).unwrap();
        assert!(meter.finished);
        assert!(meter.usage_complete);
        assert_eq!(meter.failed, terminal == "response.failed");
        assert_eq!(meter.usage().unwrap().total_tokens().unwrap(), Some(16));
    }
}

#[test]
fn fragmented_responses_frames_preserve_utf8_tools_and_terminal_usage() {
    let events = [
        json!({"type":"response.output_text.delta","delta":"中文"}),
        json!({"type":"response.reasoning_summary_text.delta","delta":"check"}),
        json!({"type":"response.output_item.added","item":{"type":"function_call","id":"call-1"}}),
        json!({"type":"response.function_call_arguments.delta","delta":"{\"key\":"}),
        json!({"type":"response.function_call_arguments.delta","delta":"1}"}),
        json!({"type":"response.completed","response":{"model":"served-model","usage":usage()}}),
    ];
    let input = events
        .iter()
        .map(|event| format!("data: {event}\r\n\r\n"))
        .collect::<String>();
    let mut meter = SseMeter::new(UsageProtocol::OpenaiResponses);
    let mut frames = Vec::new();
    for byte in input.bytes() {
        frames.extend(meter.push(&[byte]).unwrap());
    }
    frames.extend(meter.finish().unwrap());
    assert_eq!(frames.len(), 6);
    assert!(frames[..5].iter().all(|frame| frame.has_output));
    assert!(frames[0].frame.contains("中文"));
    assert!(meter.meter.finished);
    assert_eq!(
        meter.meter.usage().unwrap().total_tokens().unwrap(),
        Some(16)
    );
}

#[test]
fn invalid_responses_usage_and_failed_plain_responses_do_not_claim_success() {
    for invalid in [json!(-1), json!("3"), json!(1.5)] {
        let mut meter = WireMeter::new(UsageProtocol::OpenaiResponses);
        assert!(meter
            .response(&json!({"usage":{"input_tokens":invalid,"output_tokens":0}}))
            .is_err());
        assert!(meter.failed);
        assert!(!meter.usage_complete);
    }
    let mut failed = WireMeter::new(UsageProtocol::OpenaiResponses);
    failed
        .response(&json!({"status":"failed","usage":usage()}))
        .unwrap();
    assert!(failed.failed);
    assert_eq!(failed.usage().unwrap().total_tokens().unwrap(), Some(16));
    let mut interrupted = SseMeter::new(UsageProtocol::OpenaiResponses);
    interrupted
        .push(b"data: {\"type\":\"response.output_text.delta\",\"delta\":\"prefix\"}\n\n")
        .unwrap();
    interrupted.finish().unwrap();
    assert!(!interrupted.meter.finished);
    assert!(!interrupted.meter.usage_complete);
}

#[test]
fn relay_done_sentinel_requires_a_terminal_response_event() {
    let mut interrupted = WireMeter::new(UsageProtocol::OpenaiResponses);
    assert!(interrupted.event("[DONE]").is_err());
    assert!(!interrupted.finished);
    let mut complete = SseMeter::new(UsageProtocol::OpenaiResponses);
    let frames = complete
        .push(
            format!(
                "data: {}\n\ndata: [DONE]\n\n",
                json!({"type":"response.completed","response":{"usage":usage()}})
            )
            .as_bytes(),
        )
        .unwrap();
    assert_eq!(frames.len(), 2);
    assert!(complete.meter.finished);
    assert!(!complete.meter.failed);
    assert_eq!(
        complete.meter.usage().unwrap().total_tokens().unwrap(),
        Some(16)
    );
}
