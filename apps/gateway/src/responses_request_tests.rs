use super::*;

#[test]
fn translates_self_contained_parallel_tool_history_and_definitions() {
    let request = serde_json::json!({
        "model":"m", "instructions":"policy", "max_output_tokens":64,
        "input":[
            {"role":"developer","content":"keep IDs"},
            {"role":"user","content":"hello"},
            {"type":"function_call","id":"item_a","call_id":"call_a","name":"lookup_a","arguments":"{ \"city\": \"上海\" }"},
            {"type":"function_call","call_id":"call_b","name":"lookup_b","arguments":"{}"},
            {"type":"function_call_output","call_id":"call_b","output":"B"},
            {"type":"function_call_output","call_id":"call_a","output":"A"},
        ],
        "tools":[{"type":"function","name":"lookup_a","description":"lookup","parameters":{"type":"object","properties":{"city":{"type":"string"}}},"strict":true}],
        "tool_choice":{"type":"function","name":"lookup_a"}, "parallel_tool_calls":true,
    });
    let body = responses_request_to_openai(&request).unwrap();
    assert_eq!(
        body["messages"],
        serde_json::json!([
            {"role":"system","content":"policy"},
            {"role":"developer","content":"keep IDs"},
            {"role":"user","content":"hello"},
            {"role":"assistant","content":null,"tool_calls":[
                {"id":"call_a","type":"function","function":{"name":"lookup_a","arguments":"{ \"city\": \"上海\" }"}},
                {"id":"call_b","type":"function","function":{"name":"lookup_b","arguments":"{}"}},
            ]},
            {"role":"tool","tool_call_id":"call_b","content":"B"},
            {"role":"tool","tool_call_id":"call_a","content":"A"},
        ])
    );
    assert_eq!(
        body["tools"][0],
        serde_json::json!({"type":"function","function":{"name":"lookup_a","description":"lookup","parameters":{"type":"object","properties":{"city":{"type":"string"}}},"strict":true}})
    );
    assert_eq!(
        body["tool_choice"],
        serde_json::json!({"type":"function","function":{"name":"lookup_a"}})
    );
    assert_eq!(body["parallel_tool_calls"], true);
    assert_eq!(body["max_tokens"], 64);
}

#[test]
fn translates_images_reasoning_effort_and_json_schema_without_flattening_parts() {
    let body = responses_request_to_openai(&serde_json::json!({
        "model":"m", "input":[{"role":"user","content":[
            {"type":"input_text","text":"look"},
            {"type":"input_image","image_url":"data:image/png;base64,AA==","detail":"low"},
        ]}],
        "reasoning":{"effort":"high"},
        "text":{"format":{"type":"json_schema","name":"result","schema":{"type":"object"},"strict":false}},
    })).unwrap();
    assert_eq!(
        body["messages"][0]["content"],
        serde_json::json!([
            {"type":"text","text":"look"},
            {"type":"image_url","image_url":{"url":"data:image/png;base64,AA==","detail":"low"}},
        ])
    );
    assert_eq!(body["reasoning_effort"], "high");
    assert_eq!(
        body["response_format"],
        serde_json::json!({"type":"json_schema","json_schema":{"name":"result","schema":{"type":"object"},"strict":false}})
    );
}

#[test]
fn streaming_chat_translation_requests_usage_explicitly() {
    let body =
        responses_request_to_openai(&serde_json::json!({"model":"m","input":"hi","stream":true}))
            .unwrap();
    assert_eq!(body["stream"], true);
    assert_eq!(
        body["stream_options"],
        serde_json::json!({"include_usage":true})
    );
}

#[test]
fn refuses_orphan_duplicate_and_unfinished_tool_history() {
    for input in [
        serde_json::json!([{"type":"function_call_output","call_id":"orphan","output":"ok"}]),
        serde_json::json!([{"type":"function_call","call_id":"c","name":"f","arguments":"{}"}]),
        serde_json::json!([
            {"type":"function_call","call_id":"c","name":"f","arguments":"{}"},
            {"type":"function_call","call_id":"c","name":"f","arguments":"{}"},
            {"type":"function_call_output","call_id":"c","output":"ok"},
        ]),
        serde_json::json!([
            {"type":"function_call","call_id":"c","name":"f","arguments":"{}"},
            {"role":"user","content":"missing output"},
            {"type":"function_call_output","call_id":"c","output":"ok"},
        ]),
        serde_json::json!([
            {"type":"function_call","call_id":"a","name":"f","arguments":"{}"},
            {"type":"function_call","call_id":"b","name":"f","arguments":"{}"},
            {"type":"function_call_output","call_id":"a","output":"ok"},
            {"type":"function_call","call_id":"c","name":"f","arguments":"{}"},
            {"type":"function_call_output","call_id":"b","output":"ok"},
            {"type":"function_call_output","call_id":"c","output":"ok"},
        ]),
    ] {
        assert!(
            responses_request_to_openai(&serde_json::json!({"model":"m","input":input})).is_err()
        );
    }
}

#[test]
fn refuses_unrepresentable_and_malformed_fields_instead_of_dropping_them() {
    for field in [
        serde_json::json!({"unknown_field":"not silently ignored"}),
        serde_json::json!({"instructions":42}),
        serde_json::json!({"max_output_tokens":"bad"}),
        serde_json::json!({"stream":"true"}),
        serde_json::json!({"tools":[{"type":"web_search"}]}),
        serde_json::json!({"tools":[{"type":"function","name":""}]}),
        serde_json::json!({"tool_choice":{"type":"function","name":""}}),
        serde_json::json!({"reasoning":{"summary":"auto"}}),
        serde_json::json!({"previous_response_id":"server-side-state"}),
        serde_json::json!({"input":[{"role":"user","content":[{"type":"input_text"}]}]}),
        serde_json::json!({"input":[{"role":"user","content":[{"type":"input_image","file_id":"requires-native-files"}]}]}),
        serde_json::json!({"input":[{"role":"user","content":"hi","id":42}]}),
        serde_json::json!({"input":[{"role":"user","content":"hi","status":"unknown"}]}),
    ] {
        let mut request = serde_json::json!({"model":"m","input":"hi"});
        request
            .as_object_mut()
            .unwrap()
            .extend(field.as_object().unwrap().clone());
        assert!(
            responses_request_to_openai(&request).is_err(),
            "accepted {field}"
        );
    }
}

#[test]
fn accepts_explicitly_unset_state_and_preserves_stateless_metadata() {
    let body = responses_request_to_openai(&serde_json::json!({"model":"m","input":"hi","previous_response_id":null,"store":false,"metadata":{"actor":"fixture"}})).unwrap();
    assert_eq!(body["store"], false);
    assert_eq!(body["metadata"], serde_json::json!({"actor":"fixture"}));
    assert!(body.get("previous_response_id").is_none());
}

#[test]
fn our_reasoning_tool_output_round_trips_into_one_assistant_turn() {
    let response = openai_response_to_responses(&serde_json::json!({
        "id":"c1","model":"m","created":42,
        "choices":[{"index":0,"message":{"role":"assistant","content":null,"reasoning_content":"think first",
            "tool_calls":[{"id":"call_a","type":"function","function":{"name":"f","arguments":"{ \"x\": 1 }"}}]},"finish_reason":"tool_calls"}],
    })).unwrap();
    let mut input = response["output"].as_array().unwrap().clone();
    input.push(
        serde_json::json!({"type":"function_call_output","call_id":"call_a","output":"result"}),
    );
    let request =
        responses_request_to_openai(&serde_json::json!({"model":"m","input":input})).unwrap();
    assert_eq!(
        request["messages"],
        serde_json::json!([
            {"role":"assistant","content":null,"reasoning_content":"think first","tool_calls":[{"id":"call_a","type":"function","function":{"name":"f","arguments":"{ \"x\": 1 }"}}]},
            {"role":"tool","tool_call_id":"call_a","content":"result"},
        ])
    );
}

#[test]
fn our_text_reasoning_and_refusal_output_preserve_their_input_types() {
    for message in [
        serde_json::json!({"role":"assistant","content":"answer","reasoning_content":"think"}),
        serde_json::json!({"role":"assistant","content":null,"refusal":"cannot answer"}),
    ] {
        let response = openai_response_to_responses(&serde_json::json!({"id":"c1","model":"m","choices":[{"index":0,"message":message,"finish_reason":"stop"}]})).unwrap();
        let request = responses_request_to_openai(
            &serde_json::json!({"model":"m","input":response["output"]}),
        )
        .unwrap();
        assert_eq!(request["messages"], serde_json::json!([message]));
    }
}

#[test]
fn assistant_parts_share_a_turn_until_a_tool_result_or_user_message() {
    let request = responses_request_to_openai(&serde_json::json!({"input":[
        {"type":"reasoning","summary":[],"content":[{"type":"reasoning_text","text":"think"}]},
        {"role":"assistant","content":[{"type":"output_text","text":"A"},{"type":"refusal","refusal":"B"}]},
        {"type":"function_call","call_id":"c","name":"f","arguments":"{}"},
        {"role":"assistant","content":"C"},
        {"type":"function_call_output","call_id":"c","output":"result"},
        {"type":"reasoning","content":[{"type":"reasoning_text","text":"next"}]},
        {"role":"assistant","content":"D"},
        {"role":"user","content":"E"},
        {"role":"assistant","content":"F"},
    ]})).unwrap();
    assert_eq!(
        request["messages"],
        serde_json::json!([
            {"role":"assistant","content":"AC","refusal":"B","reasoning_content":"think","tool_calls":[{"id":"c","type":"function","function":{"name":"f","arguments":"{}"}}]},
            {"role":"tool","tool_call_id":"c","content":"result"},
            {"role":"assistant","content":"D","reasoning_content":"next"},
            {"role":"user","content":"E"},
            {"role":"assistant","content":"F"},
        ])
    );
}

#[test]
fn refuses_opaque_reasoning_and_refusal_outside_assistant_messages() {
    for item in [
        serde_json::json!({"type":"reasoning","encrypted_content":"opaque","summary":[],"content":[]}),
        serde_json::json!({"type":"reasoning","summary":[{"type":"summary_text","text":"summary is not the original reasoning"}],"content":[]}),
        serde_json::json!({"type":"reasoning","summary":[]}),
        serde_json::json!({"type":"reasoning","summary":[],"content":[]}),
        serde_json::json!({"type":"reasoning","content":[{"type":"summary_text","text":"not reasoning"}]}),
        serde_json::json!({"role":"user","content":[{"type":"refusal","refusal":"not user text"}]}),
    ] {
        assert!(responses_request_to_openai(&serde_json::json!({"input":[item]})).is_err());
    }
}

#[test]
fn reasoning_and_assistant_text_cannot_interrupt_parallel_tool_results() {
    for item in [
        serde_json::json!({"type":"reasoning","content":[{"type":"reasoning_text","text":"next turn"}]}),
        serde_json::json!({"role":"assistant","content":"next turn"}),
    ] {
        assert!(responses_request_to_openai(&serde_json::json!({"input":[
            {"type":"function_call","call_id":"a","name":"f","arguments":"{}"},
            {"type":"function_call","call_id":"b","name":"f","arguments":"{}"},
            {"type":"function_call_output","call_id":"a","output":"A"},
            item,
            {"type":"function_call_output","call_id":"b","output":"B"},
        ]}))
        .is_err());
    }
}
