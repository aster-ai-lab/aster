#![cfg(test)]

use super::*;
use serde_json::json;

#[test]
fn chat_usage_chunk_owns_final_totals_without_adding_repeated_snapshots() {
    let mut meter = WireMeter::new(UsageProtocol::OpenaiChat);
    assert!(meter
        .event(r#"{"model":"served-model","choices":[{"delta":{"content":"hello"}}],"usage":null}"#)
        .unwrap());
    assert!(!meter.usage_complete);
    assert_eq!(meter.usage().unwrap().total_tokens().unwrap(), None);
    let usage = r#"{"choices":[],"usage":{"prompt_tokens":100,"completion_tokens":20,"prompt_tokens_details":{"cached_tokens":80,"cache_write_tokens":10}}}"#;
    meter.event(usage).unwrap();
    meter.event(usage).unwrap();
    assert!(meter.usage_complete);
    assert!(!meter.finished);
    assert_eq!(meter.usage().unwrap().total_tokens().unwrap(), Some(120));
    assert_eq!(meter.usage().unwrap().no_cache_tokens, Some(10));
    meter.event("[DONE]").unwrap();
    assert!(meter.finished);
    assert_eq!(meter.response_model.as_deref(), Some("served-model"));
}

#[test]
fn absent_cache_fields_are_unknown_even_when_total_tokens_are_known() {
    let mut meter = WireMeter::new(UsageProtocol::OpenaiChat);
    meter
        .response(&json!({"usage":{"prompt_tokens":100,"completion_tokens":20}}))
        .unwrap();
    let usage = meter.usage().unwrap();
    assert_eq!(usage.total_tokens().unwrap(), Some(120));
    assert_eq!(usage.cache_read_tokens, None);
    assert_eq!(usage.no_cache_tokens, None);
    assert!(meter.usage_complete);
}

#[test]
fn anthropic_cumulative_updates_replace_counters_and_remain_incomplete_until_stop() {
    let mut meter = WireMeter::new(UsageProtocol::AnthropicMessages);
    meter.event(r#"{"type":"message_start","message":{"model":"served","usage":{"input_tokens":10,"output_tokens":1,"cache_creation_input_tokens":20,"cache_read_input_tokens":70}}}"#).unwrap();
    meter
        .event(r#"{"type":"message_delta","usage":{"output_tokens":15}}"#)
        .unwrap();
    meter
        .event(r#"{"type":"message_delta","usage":{"input_tokens":12,"output_tokens":25}}"#)
        .unwrap();
    assert!(!meter.usage_complete);
    assert_eq!(meter.usage().unwrap().total_tokens().unwrap(), Some(127));
    meter.event(r#"{"type":"message_stop"}"#).unwrap();
    assert!(meter.usage_complete);
    assert_eq!(meter.usage().unwrap().input_tokens, Some(102));
    assert_eq!(meter.usage().unwrap().output_tokens, Some(25));
    assert!(meter
        .event(r#"{"type":"message_delta","usage":{"output_tokens":24}}"#)
        .is_err());
    assert!(!meter.usage_complete);
    assert!(meter.failed);
}

#[test]
fn tool_and_thinking_deltas_are_output_but_pings_and_headers_are_not() {
    let mut meter = WireMeter::new(UsageProtocol::AnthropicMessages);
    assert!(!meter.event(r#"{"type":"ping"}"#).unwrap());
    assert!(!meter
        .event(r#"{"type":"content_block_start","content_block":{"type":"text","text":""}}"#)
        .unwrap());
    assert!(meter.event(r#"{"type":"content_block_start","content_block":{"type":"tool_use","name":"read_file"}}"#).unwrap());
    assert!(meter.event(r#"{"type":"content_block_delta","delta":{"type":"thinking_delta","thinking":"checking"}}"#).unwrap());
    assert!(meter.event(r#"{"type":"content_block_delta","delta":{"type":"input_json_delta","partial_json":"{"}}"#).unwrap());
    assert!(!meter.event(r#"{"type":"future_event"}"#).unwrap());
    meter
        .event(r#"{"type":"error","error":{"type":"overloaded_error"}}"#)
        .unwrap();
    assert!(meter.failed);
    assert!(!meter.usage_complete);
}

#[test]
fn invalid_counters_and_overlapping_cache_usage_are_refused() {
    for usage in [
        json!({"prompt_tokens":-1}),
        json!({"completion_tokens":"20"}),
        json!({"prompt_tokens":10,"prompt_tokens_details":{"cached_tokens":20,"cache_write_tokens":0}}),
        json!({"prompt_tokens_details": []}),
    ] {
        assert!(WireMeter::new(UsageProtocol::OpenaiChat)
            .response(&json!({"usage":usage}))
            .is_err());
    }
    assert!(UsageProtocol::parse("auto").is_err());
    for value in ["openai_chat", "anthropic_messages"] {
        assert_eq!(UsageProtocol::parse(value).unwrap().as_str(), value);
    }
}

#[test]
fn sse_meter_preserves_split_utf8_multiline_data_and_bounded_frames() {
    let input = "data: {\"choices\":\r\ndata: [{\"delta\":{\"content\":\"中文\"}}]}\r\n\r\ndata: [DONE]\r\n\r\n";
    let mut meter = SseMeter::new(UsageProtocol::OpenaiChat);
    let mut observed = Vec::new();
    for byte in input.bytes() {
        observed.extend(meter.push(&[byte]).unwrap());
    }
    observed.extend(meter.finish().unwrap());
    assert_eq!(observed.len(), 2);
    assert!(observed[0].has_output);
    assert!(!observed[1].has_output);
    assert!(observed[0].frame.contains("中文"));
    assert!(meter.meter.finished);
    assert!(!meter.meter.usage_complete);
    assert!(SseMeter::new(UsageProtocol::OpenaiChat)
        .push(&vec![b'x'; 1024 * 1024 + 1])
        .is_err());
}

#[test]
fn dispatch_endpoints_keep_the_existing_provider_url_rules() {
    assert_eq!(
        UsageProtocol::OpenaiChat.endpoint("https://api.example.test/v1"),
        "https://api.example.test/v1/chat/completions"
    );
    assert_eq!(
        UsageProtocol::OpenaiChat.endpoint("https://api.example.test"),
        "https://api.example.test/v1/chat/completions"
    );
    assert_eq!(
        UsageProtocol::OpenaiChat.endpoint("https://api.example.test/custom/v4/"),
        "https://api.example.test/custom/v4/chat/completions"
    );
    assert_eq!(
        UsageProtocol::AnthropicMessages.endpoint("https://api.example.test/v1/"),
        "https://api.example.test/v1/messages"
    );
    assert_eq!(
        UsageProtocol::AnthropicMessages.endpoint("https://api.example.test/apps/anthropic"),
        "https://api.example.test/apps/anthropic/v1/messages"
    );
}

#[test]
fn malformed_framing_invalidates_previously_complete_usage() {
    for tail in [vec![0xff], vec![b'x'; 1024 * 1024 + 1]] {
        let mut meter = SseMeter::new(UsageProtocol::OpenaiChat);
        meter
            .push(b"data: {\"usage\":{\"prompt_tokens\":10,\"completion_tokens\":2}}\n\n")
            .unwrap();
        assert!(meter.meter.usage_complete);
        assert!(meter.push(&tail).is_err());
        assert!(meter.meter.failed);
        assert!(!meter.meter.usage_complete);
    }
    let mut meter = SseMeter::new(UsageProtocol::OpenaiChat);
    meter
        .push(b"data: {\"usage\":{\"prompt_tokens\":10,\"completion_tokens\":2}}\n\n")
        .unwrap();
    meter.push(&[0xe4]).unwrap();
    assert!(meter.finish().is_err());
    assert!(!meter.meter.usage_complete);
}
