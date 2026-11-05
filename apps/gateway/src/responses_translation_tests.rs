use super::*;

#[test]
fn responses_json_uses_the_same_typed_output_and_cache_usage() {
    let response = openai_response_to_responses(&serde_json::json!({
        "id":"c1", "model":"m", "created":42,
        "choices":[{"index":0,"message":{"role":"assistant","content":null,"reasoning_content":"reason",
            "tool_calls":[{"id":"call_a","type":"function","function":{"name":"f","arguments":"{\"city\":\"上海\"}"}}]},
            "finish_reason":"tool_calls"}],
        "usage":{"prompt_tokens":13,"completion_tokens":8,"total_tokens":21,"prompt_tokens_details":{"cached_tokens":5},"completion_tokens_details":{"reasoning_tokens":3}},
    })).unwrap();
    assert_eq!(response["output"][0]["type"], "reasoning");
    assert_eq!(response["output"][1]["type"], "function_call");
    assert_eq!(response["output"][1]["call_id"], "call_a");
    assert_eq!(response["output"][1]["arguments"], "{\"city\":\"上海\"}");
    assert_eq!(
        response["usage"]["input_tokens_details"]["cached_tokens"],
        5
    );
    assert_eq!(
        response["usage"]["output_tokens_details"]["reasoning_tokens"],
        3
    );
}

fn chunk(delta: Value, finish: Value) -> Value {
    serde_json::json!({
        "id": "chat-1", "object": "chat.completion.chunk", "created": 42, "model": "fixture/model",
        "choices": [{"index": 0, "delta": delta, "finish_reason": finish}],
    })
}

#[test]
fn responses_waits_for_trailing_usage_and_emits_one_terminal() {
    let mut machine = ChatToResponsesStream::new();
    machine
        .push(&chunk(serde_json::json!({"content":"hello"}), Value::Null))
        .unwrap();
    let pending = machine
        .push(&chunk(serde_json::json!({}), Value::String("stop".into())))
        .unwrap();
    assert!(!machine.is_finished());
    assert!(!pending
        .iter()
        .any(|event| event["type"] == "response.completed"));
    let events = machine.push(&serde_json::json!({"choices":[], "usage":{
        "prompt_tokens":13, "completion_tokens":8, "total_tokens":21,
        "prompt_tokens_details":{"cached_tokens":5}, "completion_tokens_details":{"reasoning_tokens":3},
    }})).unwrap();
    let terminal = events
        .iter()
        .filter(|event| event["type"] == "response.completed")
        .collect::<Vec<_>>();
    assert_eq!(terminal.len(), 1);
    assert_eq!(
        terminal[0]["response"]["usage"],
        serde_json::json!({
            "input_tokens":13, "output_tokens":8, "total_tokens":21,
            "input_tokens_details":{"cached_tokens":5}, "output_tokens_details":{"reasoning_tokens":3},
        })
    );
    assert!(machine
        .push(&Value::String("[DONE]".into()))
        .unwrap()
        .is_empty());
}

#[test]
fn responses_preserves_interleaved_tool_ids_and_arguments() {
    let events = translate_chat_to_responses_stream(&[
        chunk(serde_json::json!({"role":"assistant"}), Value::Null),
        chunk(serde_json::json!({"tool_calls":[
            {"index":1,"id":"call_b","type":"function","function":{"name":"lookup_b","arguments":"{\"b\":"}},
            {"index":0,"id":"call_a","type":"function","function":{"name":"lookup_a","arguments":"{\"a\":"}},
        ]}), Value::Null),
        chunk(serde_json::json!({"tool_calls":[
            {"index":0,"function":{"arguments":"\"上海\"}"}},
            {"index":1,"function":{"arguments":"2}"}},
        ]}), Value::Null),
        chunk(serde_json::json!({}), Value::String("tool_calls".into())),
        serde_json::json!({"choices":[],"usage":{"prompt_tokens":3,"completion_tokens":4,"total_tokens":7}}),
        Value::String("[DONE]".into()),
    ]).unwrap();
    let terminal = events
        .iter()
        .find(|event| event["type"] == "response.completed")
        .unwrap();
    let output = terminal["response"]["output"].as_array().unwrap();
    assert_eq!(output.len(), 2);
    for (index, (id, name, arguments)) in [
        ("call_b", "lookup_b", "{\"b\":2}"),
        ("call_a", "lookup_a", "{\"a\":\"上海\"}"),
    ]
    .into_iter()
    .enumerate()
    {
        assert_eq!(output[index]["type"], "function_call");
        assert_eq!(output[index]["call_id"], id);
        assert_eq!(output[index]["name"], name);
        assert_eq!(output[index]["arguments"], arguments);
        let deltas = events
            .iter()
            .filter(|event| {
                event["type"] == "response.function_call_arguments.delta"
                    && event["output_index"] == index
            })
            .map(|event| event["delta"].as_str().unwrap())
            .collect::<String>();
        assert_eq!(deltas, arguments);
        assert_eq!(
            events
                .iter()
                .filter(
                    |event| event["type"] == "response.function_call_arguments.done"
                        && event["output_index"] == index
                )
                .count(),
            1
        );
    }
}

#[test]
fn responses_done_without_a_finish_reason_is_not_success() {
    let mut machine = ChatToResponsesStream::new();
    machine
        .push(&chunk(
            serde_json::json!({"content":"partial"}),
            Value::Null,
        ))
        .unwrap();
    assert!(machine.push(&Value::String("[DONE]".into())).is_err());
}

#[test]
fn responses_length_limit_uses_the_incomplete_terminal_event() {
    let events = translate_chat_to_responses_stream(&[
        chunk(serde_json::json!({"content":"partial"}), Value::Null),
        chunk(serde_json::json!({}), Value::String("length".into())),
        Value::String("[DONE]".into()),
    ])
    .unwrap();
    assert_eq!(
        events
            .iter()
            .filter(|event| event["type"] == "response.incomplete")
            .count(),
        1
    );
    assert!(!events
        .iter()
        .any(|event| event["type"] == "response.completed"));
    let response = &events.last().unwrap()["response"];
    assert_eq!(response["status"], "incomplete");
    assert_eq!(
        response["incomplete_details"]["reason"],
        "max_output_tokens"
    );
}

#[test]
fn responses_preserves_reasoning_and_refusal_as_typed_items() {
    let events = translate_chat_to_responses_stream(&[
        chunk(
            serde_json::json!({"reasoning_content":"think first"}),
            Value::Null,
        ),
        chunk(serde_json::json!({"refusal":"cannot answer"}), Value::Null),
        chunk(
            serde_json::json!({}),
            Value::String("content_filter".into()),
        ),
        Value::String("[DONE]".into()),
    ])
    .unwrap();
    assert!(events
        .iter()
        .any(|event| event["type"] == "response.reasoning_text.delta"));
    assert!(events
        .iter()
        .any(|event| event["type"] == "response.refusal.delta"));
    let response = &events.last().unwrap()["response"];
    assert_eq!(response["output"][0]["type"], "reasoning");
    assert_eq!(response["output"][0]["content"][0]["text"], "think first");
    assert_eq!(
        response["output"][1]["content"][0]["refusal"],
        "cannot answer"
    );
    assert_eq!(response["incomplete_details"]["reason"], "content_filter");
}

#[test]
fn responses_refuses_unsupported_or_oversized_output() {
    for frame in [
        chunk(
            serde_json::json!({"audio":{"data":"unsupported"}}),
            Value::Null,
        ),
        chunk(serde_json::json!({}), Value::String("unknown".into())),
        chunk(
            serde_json::json!({"tool_calls":[{"index":0,"function":{"arguments":"{}"}}]}),
            Value::Null,
        ),
        chunk(
            serde_json::json!({"content":"x".repeat(MAX_RESPONSES_OUTPUT_BYTES + 1)}),
            Value::Null,
        ),
    ] {
        assert!(ChatToResponsesStream::new().push(&frame).is_err());
    }
    let mut machine = ChatToResponsesStream::new();
    for index in 0..MAX_RESPONSES_OUTPUT_ITEMS {
        machine.push(&chunk(serde_json::json!({"tool_calls":[{"index":index,"id":format!("call_{index}"),"function":{"name":"f","arguments":""}}]}), Value::Null)).unwrap();
    }
    assert!(machine.push(&chunk(serde_json::json!({"tool_calls":[{"index":MAX_RESPONSES_OUTPUT_ITEMS,"id":"too-many","function":{"name":"f"}}]}), Value::Null)).is_err());
}

#[test]
fn responses_debug_never_contains_payloads_or_identifiers() {
    const SECRET: &str = "synthetic-private-payload";
    let mut frame = chunk(
        serde_json::json!({"content":SECRET,"tool_calls":[{"index":0,"id":SECRET,"function":{"name":SECRET,"arguments":SECRET}}]}),
        Value::Null,
    );
    frame["id"] = Value::String(SECRET.into());
    frame["model"] = Value::String(SECRET.into());
    let mut machine = ChatToResponsesStream::new();
    machine.push(&frame).unwrap();
    let debug = format!("{machine:?}");
    assert!(!debug.contains(SECRET));
    assert!(debug.contains("output_items"));
    let mut anthropic = OpenaiToAnthropicStream::new();
    frame["usage"] = serde_json::json!({"vendor_metadata":SECRET});
    anthropic.push(&frame).unwrap();
    assert!(!format!("{anthropic:?}").contains(SECRET));
}
