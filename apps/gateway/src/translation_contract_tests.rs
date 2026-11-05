//! PT0 translator contract tests: the frozen corpus is the expected output.
//!
//! Forward translation must reproduce the fixture's Anthropic body exactly;
//! reverse translation must reproduce the OpenAI body with the ledgered
//! (untranslatable) top-level fields removed. Failures here are translator
//! bugs, never corpus edits.

use super::*;
use serde_json::Value;

#[test]
fn gemini_counter_estimates_preserve_context_and_ignore_output_settings() {
    let contents = serde_json::json!({"contents":[{"parts":[{"text":"hello"}]}]});
    let base = estimate_gemini_input_tokens(&contents).unwrap();
    let mut full = contents.clone();
    full["systemInstruction"] =
        serde_json::json!({"parts":[{"text":"long instructions ".repeat(100)}]});
    full["tools"] = serde_json::json!([{"functionDeclarations":[{"name":"lookup","description":"function context ".repeat(100)}]}]);
    full["cachedContent"] = serde_json::json!("cachedContents/context");
    let with_context = estimate_gemini_input_tokens(&full).unwrap();
    assert!(with_context > base);
    for field in ["generateContentRequest", "generate_content_request"] {
        let wrapped = serde_json::json!({field:full});
        assert_eq!(
            estimate_gemini_input_tokens(&wrapped).unwrap(),
            with_context
        );
        assert_eq!(gemini_count_input(&wrapped).unwrap(), &full);
    }
    full["model"] = serde_json::json!("models/ignored");
    full["stream"] = serde_json::json!(true);
    full["generationConfig"] = serde_json::json!({"maxOutputTokens":9000,"temperature":0.5});
    assert_eq!(estimate_gemini_input_tokens(&full).unwrap(), with_context);
}

#[test]
fn gemini_counter_refuses_ambiguous_and_malformed_input() {
    for request in [
        serde_json::json!(null),
        serde_json::json!([]),
        serde_json::json!({"contents":null}),
        serde_json::json!({"contents":[true]}),
        serde_json::json!({"contents":[{"parts":true}]}),
        serde_json::json!({"contents":[{"parts":[true]}]}),
        serde_json::json!({"generateContentRequest":true}),
        serde_json::json!({"contents":[],"generateContentRequest":{"contents":[]}}),
        serde_json::json!({"generateContentRequest":{"contents":[]},"generate_content_request":{"contents":[]}}),
    ] {
        assert!(estimate_gemini_input_tokens(&request).is_err(), "{request}");
    }
    assert!(estimate_gemini_input_tokens(&serde_json::json!({"contents":[]})).is_ok());
    assert!(estimate_gemini_input_tokens(&serde_json::json!({})).is_ok());
    assert!(estimate_gemini_input_tokens(&serde_json::json!({"generateContentRequest":{"systemInstruction":{"parts":[{"text":"instructions"}]}}})).is_ok());
}

const PLAIN_CHAT: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/tests/fixtures/translation/plain_chat_pair.json"
));
const TOOLS_ROUNDTRIP: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/tests/fixtures/translation/openai_tools_roundtrip.json"
));
const MULTIMODAL: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/tests/fixtures/translation/multimodal_passthrough_pair.json"
));
const OPENAI_LOSS: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/tests/fixtures/translation/openai_untranslatable_logprobs.json"
));
const ANTHROPIC_LOSS: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/tests/fixtures/translation/anthropic_thinking_no_openai_equivalent.json"
));

fn corpus() -> Vec<(&'static str, Value)> {
    [
        PLAIN_CHAT,
        TOOLS_ROUNDTRIP,
        MULTIMODAL,
        OPENAI_LOSS,
        ANTHROPIC_LOSS,
    ]
    .into_iter()
    .map(|raw| {
        let fixture: Value = serde_json::from_str(raw).expect("corpus fixture JSON");
        let name = fixture["name"].as_str().expect("name").to_string();
        (name.leak() as &'static str, fixture)
    })
    .collect()
}

/// Top-level ledgered field names ("field: reason" entries).
fn ledgered_top_level_fields(fixture: &Value) -> Vec<String> {
    fixture["loss_ledger_entries"]
        .as_array()
        .map(|entries| {
            entries
                .iter()
                .map(|entry| {
                    entry
                        .as_str()
                        .and_then(|text| text.split_once(':'))
                        .map(|(field, _)| field.trim().to_string())
                        .unwrap_or_default()
                })
                .collect()
        })
        .unwrap_or_default()
}

/// A ledgered field exists on exactly one side of the pair, so removing the
/// ledgered names from both bodies yields the translatable cores.
fn translatable_core(body: &Value, fixture: &Value) -> Value {
    let mut core = body.clone();
    for field in ledgered_top_level_fields(fixture) {
        core.as_object_mut()
            .expect("corpus body object")
            .remove(&field);
    }
    core
}

/// Canonical form for comparison: object keys sort recursively and
/// `function.arguments` — a JSON object encoded as a string — compares by
/// its parsed value, not by re-serialization spacing.
fn canonical(value: &Value) -> Value {
    match value {
        Value::Object(object) => {
            let mut sorted = serde_json::Map::new();
            let mut keys = object.keys().collect::<Vec<_>>();
            keys.sort();
            for key in keys {
                sorted.insert(key.clone(), canonical(&object[key]));
            }
            if sorted.get("arguments").is_some_and(|args| args.is_string()) {
                if let Ok(parsed) =
                    serde_json::from_str::<Value>(sorted["arguments"].as_str().expect("string"))
                {
                    sorted.insert("arguments".into(), canonical(&parsed));
                }
            }
            Value::Object(sorted)
        }
        Value::Array(items) => Value::Array(items.iter().map(canonical).collect()),
        other => other.clone(),
    }
}

#[test]
fn forward_translation_reproduces_the_frozen_anthropic_bodies() {
    for (name, fixture) in corpus() {
        let translated = openai_request_to_anthropic(&fixture["openai"]).expect(name);
        assert_eq!(
            canonical(&translated),
            canonical(&translatable_core(&fixture["anthropic"], &fixture)),
            "{name}: forward drift"
        );
    }
}

#[test]
fn reverse_translation_reproduces_the_openai_bodies_minus_ledgered_fields() {
    for (name, fixture) in corpus() {
        let translated = anthropic_request_to_openai(&fixture["anthropic"]).expect(name);
        assert_eq!(
            canonical(&translated),
            canonical(&translatable_core(&fixture["openai"], &fixture)),
            "{name}: reverse drift beyond the ledger"
        );
    }
}

#[test]
fn round_trip_through_both_dialects_is_stable() {
    for (name, fixture) in corpus() {
        let anthropic = openai_request_to_anthropic(&fixture["openai"]).expect(name);
        let back = anthropic_request_to_openai(&anthropic).expect(name);
        assert_eq!(
            canonical(&back),
            canonical(&translatable_core(&fixture["openai"], &fixture)),
            "{name}: round-trip is not stable"
        );
    }
}

#[test]
fn unknown_roles_refuse_with_a_field_error() {
    let request = serde_json::json!({
        "model": "chat-model",
        "messages": [{ "role": "developer", "content": "hi" }],
    });
    let error = openai_request_to_anthropic(&request).unwrap_err();
    assert!(error.0.contains("message.role"));
    assert!(error.0.contains("developer"));
}

#[test]
fn mid_conversation_system_message_refuses() {
    let request = serde_json::json!({
        "model": "chat-model",
        "messages": [
            { "role": "user", "content": "hi" },
            { "role": "system", "content": "late" },
        ],
    });
    let error = openai_request_to_anthropic(&request).unwrap_err();
    assert!(error.0.contains("system message after"));
}

#[test]
fn broken_tool_chain_refuses() {
    let request = serde_json::json!({
        "model": "chat-model",
        "messages": [
            { "role": "user", "content": "hi" },
            { "role": "tool", "tool_call_id": "call_missing", "content": "{}" },
        ],
    });
    let error = openai_request_to_anthropic(&request).unwrap_err();
    assert!(error.0.contains("tool_call"));
}

#[test]
fn malformed_tool_arguments_refuse() {
    let request = serde_json::json!({
        "model": "chat-model",
        "messages": [
            { "role": "user", "content": "hi" },
            {
                "role": "assistant",
                "content": null,
                "tool_calls": [{
                    "id": "call_1",
                    "type": "function",
                    "function": { "name": "f", "arguments": "not-json" }
                }]
            },
        ],
    });
    let error = openai_request_to_anthropic(&request).unwrap_err();
    assert!(error.0.contains("tool_calls.function.arguments"));
}

#[test]
fn non_data_url_image_refuses() {
    let request = serde_json::json!({
        "model": "chat-model",
        "messages": [{
            "role": "user",
            "content": [
                { "type": "text", "text": "look" },
                { "type": "image_url", "image_url": { "url": "https://example.com/x.png" } }
            ]
        }],
    });
    let error = openai_request_to_anthropic(&request).unwrap_err();
    assert!(error.0.contains("image_url.url"));
}

#[test]
fn anthropic_tool_use_without_object_input_refuses() {
    let request = serde_json::json!({
        "model": "chat-model",
        "max_tokens": 32,
        "messages": [{
            "role": "assistant",
            "content": [{ "type": "tool_use", "id": "call_1", "name": "f", "input": 7 }]
        }],
    });
    let error = anthropic_request_to_openai(&request).unwrap_err();
    assert!(error.0.contains("tool_use.input"));
}

const PLAIN_RESPONSE: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/tests/fixtures/translation/responses/plain_chat_response_pair.json"
));
const TOOL_RESPONSE: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/tests/fixtures/translation/responses/tool_call_response_pair.json"
));

fn response_corpus() -> Vec<(&'static str, Value)> {
    [PLAIN_RESPONSE, TOOL_RESPONSE]
        .into_iter()
        .map(|raw| {
            let fixture: Value = serde_json::from_str(raw).expect("response fixture JSON");
            let name = fixture["name"].as_str().expect("name").to_string();
            (name.leak() as &'static str, fixture)
        })
        .collect()
}

#[test]
fn anthropic_responses_translate_back_to_the_frozen_openai_shapes() {
    for (name, fixture) in response_corpus() {
        let created = fixture["openai"]["created"].as_i64().expect("created");
        let translated = anthropic_response_to_openai(&fixture["anthropic"], created).expect(name);
        assert_eq!(
            canonical(&translated),
            canonical(&fixture["openai"]),
            "{name}: response drift"
        );
    }
}

#[test]
fn openai_responses_translate_back_to_the_frozen_anthropic_shapes() {
    for (name, fixture) in response_corpus() {
        let translated = openai_response_to_anthropic(&fixture["openai"]).expect(name);
        assert_eq!(
            canonical(&translated),
            canonical(&fixture["anthropic"]),
            "{name}: response drift"
        );
    }
}

#[test]
fn unknown_stop_and_finish_reasons_refuse() {
    let bad_anthropic = serde_json::json!({
        "id": "resp_1",
        "type": "message",
        "role": "assistant",
        "model": "m",
        "content": [{ "type": "text", "text": "hi" }],
        "stop_reason": "refusal",
        "usage": { "input_tokens": 1, "output_tokens": 1 }
    });
    assert!(anthropic_response_to_openai(&bad_anthropic, 0)
        .unwrap_err()
        .0
        .contains("stop_reason"));

    let bad_openai = serde_json::json!({
        "id": "resp_1",
        "object": "chat.completion",
        "created": 0,
        "model": "m",
        "choices": [{
            "index": 0,
            "message": { "role": "assistant", "content": "hi" },
            "finish_reason": "content_filter"
        }],
        "usage": { "prompt_tokens": 1, "completion_tokens": 1, "total_tokens": 2 }
    });
    assert!(openai_response_to_anthropic(&bad_openai)
        .unwrap_err()
        .0
        .contains("finish_reason"));
}

#[test]
fn cache_lanes_round_trip_through_both_usage_shapes() {
    let anthropic = serde_json::json!({
        "input_tokens": 10, "cache_read_input_tokens": 4,
        "cache_creation_input_tokens": 2, "output_tokens": 6,
    });
    let openai = super::anthropic_usage_to_openai(&anthropic).expect("usage");
    assert_eq!(openai["prompt_tokens"], 16);
    assert_eq!(openai["prompt_tokens_details"]["cached_tokens"], 4);
    let back = super::openai_usage_to_anthropic(&openai).expect("usage");
    assert_eq!(back["input_tokens"], 12);
    assert_eq!(back["cache_read_input_tokens"], 4);
    assert_eq!(back["output_tokens"], 6);
}

const PLAIN_STREAM: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/tests/fixtures/translation/streams/plain_text_stream_pair.json"
));
const TOOL_STREAM: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/tests/fixtures/translation/streams/tool_use_stream_pair.json"
));

fn stream_corpus() -> Vec<(&'static str, Value)> {
    [PLAIN_STREAM, TOOL_STREAM]
        .into_iter()
        .map(|raw| {
            let fixture: Value = serde_json::from_str(raw).expect("stream fixture JSON");
            let name = fixture["name"].as_str().expect("name").to_string();
            (name.leak() as &'static str, fixture)
        })
        .collect()
}

fn event_list(fixture: &Value, dialect: &str) -> Vec<Value> {
    fixture[dialect].as_array().expect("event list").clone()
}

#[test]
fn anthropic_streams_translate_to_the_frozen_openai_frames() {
    for (name, fixture) in stream_corpus() {
        let created = event_list(&fixture, "openai")
            .first()
            .and_then(|frame| frame.get("created"))
            .and_then(Value::as_i64)
            .expect("created");
        let translated =
            translate_anthropic_stream(&event_list(&fixture, "anthropic"), created).expect(name);
        let expected = event_list(&fixture, "openai");
        assert_eq!(translated.len(), expected.len(), "{name}: frame count");
        for (index, (got, want)) in translated.iter().zip(&expected).enumerate() {
            if want.is_string() {
                assert_eq!(got, want, "{name}: sentinel at {index}");
                continue;
            }
            assert_eq!(canonical(got), canonical(want), "{name}: frame {index}");
        }
    }
}

#[test]
fn stream_translation_refuses_unknown_events_and_post_stop_input() {
    let unknown = serde_json::json!({ "type": "content_block_pause" });
    let error = AnthropicToOpenaiStream::new(0).push(&unknown).unwrap_err();
    assert!(error.0.contains("message_start, ping, content_block_start"));

    let mut machine = AnthropicToOpenaiStream::new(0);
    machine
        .push(&serde_json::json!({ "type": "message_stop" }))
        .unwrap();
    let error = machine
        .push(&serde_json::json!({ "type": "message_stop" }))
        .unwrap_err();
    assert!(error.0.contains("already stopped"));

    let orphan_delta = serde_json::json!({
        "type": "content_block_delta",
        "index": 3,
        "delta": { "type": "input_json_delta", "partial_json": "{}" },
    });
    let error = AnthropicToOpenaiStream::new(0)
        .push(&orphan_delta)
        .unwrap_err();
    assert!(error.0.contains("not a tool_use"));
}

#[test]
fn openai_streams_translate_back_to_the_frozen_anthropic_events() {
    for (name, fixture) in stream_corpus() {
        let translated = translate_openai_stream(&event_list(&fixture, "openai")).expect(name);
        let expected = event_list(&fixture, "anthropic");
        assert_eq!(translated.len(), expected.len(), "{name}: event count");
        for (index, (got, want)) in translated.iter().zip(&expected).enumerate() {
            let mut want = want.clone();
            if want["type"] == "message_start" {
                // Directional deviation: the reverse machine cannot know
                // input tokens before the terminal usage frame, so
                // message_start carries provisional zeroes.
                want["message"]["usage"] = serde_json::json!({
                    "input_tokens": 0,
                    "output_tokens": 0,
                });
            }
            assert_eq!(canonical(got), canonical(&want), "{name}: event {index}");
        }
    }
}

#[test]
fn reverse_stream_refuses_post_terminal_input_and_bad_sentinels() {
    let mut machine = OpenaiToAnthropicStream::new();
    machine
        .push(&serde_json::json!({
            "id": "r", "object": "chat.completion.chunk", "created": 0, "model": "m",
            "choices": [{ "index": 0, "delta": {}, "finish_reason": "stop" }],
        }))
        .unwrap();
    assert!(!machine.is_finished());
    machine.push(&Value::String("[DONE]".into())).unwrap();
    assert!(machine.is_finished());
    let error = machine
        .push(&serde_json::json!({
            "id": "r", "object": "chat.completion.chunk", "created": 0, "model": "m",
            "choices": [{ "index": 0, "delta": {}, "finish_reason": null }],
        }))
        .unwrap_err();
    assert!(error.0.contains("already stopped"));

    let error = OpenaiToAnthropicStream::new()
        .push(&Value::String("not-done".into()))
        .unwrap_err();
    assert!(error.0.contains("[DONE]"));
}

#[test]
fn anthropic_string_content_shorthand_translates() {
    let request = serde_json::json!({
        "model": "chat-model",
        "max_tokens": 32,
        "stream": true,
        "messages": [{ "role": "user", "content": "plain shorthand" }],
    });
    let translated = anthropic_request_to_openai(&request).unwrap();
    assert_eq!(translated["messages"][0]["content"], "plain shorthand");
}

const RESPONSES_PLAIN: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/tests/fixtures/translation/responses/plain_responses_pair.json"
));
const RESPONSES_ITEMS: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/tests/fixtures/translation/responses/responses_items_pair.json"
));
const RESPONSES_RESPONSE: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/tests/fixtures/translation/responses/plain_responses_response_pair.json"
));

#[test]
fn responses_requests_translate_to_the_frozen_chat_shapes() {
    for raw in [RESPONSES_PLAIN, RESPONSES_ITEMS] {
        let fixture: Value = serde_json::from_str(raw).expect("responses fixture JSON");
        let name = fixture["name"].as_str().expect("name");
        let translated = responses_request_to_openai(&fixture["responses"]).expect(name);
        assert_eq!(
            canonical(&translated),
            canonical(&fixture["openai"]),
            "{name}: request drift"
        );
    }
}

#[test]
fn chat_completions_translate_back_to_the_frozen_response_objects() {
    let fixture: Value = serde_json::from_str(RESPONSES_RESPONSE).expect("fixture");
    let translated = openai_response_to_responses(&fixture["openai"]).expect("response");
    assert_eq!(
        canonical(&translated),
        canonical(&fixture["responses"]),
        "response drift"
    );
}

#[test]
fn stateful_responses_surfaces_refuse_closed() {
    let stateful = serde_json::json!({
        "model": "chat-model",
        "previous_response_id": "resp_0",
        "input": "continue",
    });
    let error = responses_request_to_openai(&stateful).unwrap_err();
    assert!(error.0.contains("previous_response_id"));
    assert!(error.0.contains("responses-native"));

    let function_item = serde_json::json!({
        "model": "chat-model",
        "input": [
            { "type": "function_call", "call_id": "c1", "name": "f", "arguments": "{}" }
        ],
    });
    let error = responses_request_to_openai(&function_item).unwrap_err();
    assert!(error.0.contains("input.type"));
    assert!(error.0.contains("function_call"));
}

#[test]
fn responses_defaults_the_completion_bound_fail_closed() {
    let request = serde_json::json!({
        "model": "chat-model",
        "input": "hi",
    });
    let translated = responses_request_to_openai(&request).unwrap();
    assert_eq!(translated["max_tokens"], 1_024);
}

const RESPONSES_STREAM: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/tests/fixtures/translation/streams/responses_plain_pair.json"
));

#[test]
fn chat_chunks_translate_to_the_frozen_responses_events() {
    let fixture: Value = serde_json::from_str(RESPONSES_STREAM).expect("fixture");
    let translated =
        translate_chat_to_responses_stream(&event_list(&fixture, "openai")).expect("stream");
    let expected = event_list(&fixture, "responses");
    assert_eq!(translated.len(), expected.len(), "event count");
    for (index, (got, want)) in translated.iter().zip(&expected).enumerate() {
        assert_eq!(canonical(got), canonical(want), "event {index}");
    }
}

#[test]
fn responses_stream_refuses_post_terminal_and_bad_sentinels() {
    let mut machine = ChatToResponsesStream::new();
    machine
        .push(&serde_json::json!({
            "id": "r", "object": "chat.completion.chunk", "created": 0, "model": "m",
            "choices": [{ "index": 0, "delta": {}, "finish_reason": "stop" }],
        }))
        .unwrap();
    assert!(!machine.is_finished());
    machine.push(&Value::String("[DONE]".into())).unwrap();
    assert!(machine.is_finished());
    let error = machine
        .push(&serde_json::json!({
            "id": "r", "object": "chat.completion.chunk", "created": 0, "model": "m",
            "choices": [{ "index": 0, "delta": { "content": "late" }, "finish_reason": null }],
        }))
        .unwrap_err();
    assert!(error.0.contains("already completed"));
    // The trailing sentinel after terminal is idempotent.
    assert!(machine
        .push(&Value::String("[DONE]".into()))
        .unwrap()
        .is_empty());
}
