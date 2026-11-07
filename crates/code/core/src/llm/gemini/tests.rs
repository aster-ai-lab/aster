use super::*;
use crate::llm::{ContentBlock, Message};
use serde_json::json;

#[path = "transport_tests.rs"]
mod transport_tests;

fn parts() -> Vec<serde_json::Value> {
    vec![
        json!({"text":"why","thought":true,"thoughtSignature":"synthetic-private-signature"}),
        json!({"text":"answer","vendorExtension":{"keep":true}}),
        json!({"functionCall":{"name":"lookup","args":{"q":"中"}},"thoughtSignature":"synthetic-tool-signature"}),
        json!({"functionCall":{"id":"vendor-call","name":"other","args":{}}}),
    ]
}

#[test]
fn native_parts_round_trip_through_existing_message_serde_and_tool_projection() {
    let message = GeminiState::message("provider-a", "stored-model", parts()).unwrap();
    assert_eq!(message.text(), "answer");
    assert_eq!(message.reasoning_content.as_deref(), Some("why"));
    let calls = message.tool_calls();
    assert_eq!(calls.len(), 2);
    assert_eq!(calls[0].name, "lookup");
    assert_eq!(calls[0].args, json!({"q":"中"}));
    assert_eq!(calls[1].id, "vendor-call");
    let restored: Message = serde_json::from_slice(&serde_json::to_vec(&message).unwrap()).unwrap();
    assert_eq!(restored.tool_calls()[0].id, calls[0].id);
    let ContentBlock::GeminiState { state } = restored.content.last().unwrap() else {
        panic!("native state missing")
    };
    assert_eq!(
        state
            .replay(&restored, "provider-a", "stored-model")
            .unwrap(),
        parts()
    );
    let debug = format!("{restored:?}");
    assert!(!debug.contains("synthetic-private-signature"));
    assert!(!debug.contains("synthetic-tool-signature"));
    assert!(!crate::compaction::render_message_for_summary(&restored).contains("signature"));
}

#[test]
fn native_replay_refuses_changed_projection_origin_or_role() {
    let message = GeminiState::message("provider-a", "stored-model", parts()).unwrap();
    let ContentBlock::GeminiState { state } = message.content.last().unwrap() else {
        panic!("native state missing")
    };
    for (provider, model) in [
        ("provider-b", "stored-model"),
        ("provider-a", "other-model"),
    ] {
        assert!(matches!(
            state.replay(&message, provider, model),
            Err(GeminiError::StateMismatch)
        ));
    }
    let mut changed = message.clone();
    changed.content.remove(0);
    assert!(state
        .replay(&changed, "provider-a", "stored-model")
        .is_err());
    changed = message.clone();
    changed.reasoning_content = None;
    assert!(state
        .replay(&changed, "provider-a", "stored-model")
        .is_err());
    changed = message.clone();
    changed.role = "user".into();
    assert!(state
        .replay(&changed, "provider-a", "stored-model")
        .is_err());
    changed = message.clone();
    if let ContentBlock::ToolUse { input, .. } = &mut changed.content[1] {
        *input = json!({"q":"changed"});
    }
    assert!(state
        .replay(&changed, "provider-a", "stored-model")
        .is_err());
}

#[test]
fn native_state_refuses_invalid_restored_call_bindings_and_part_shapes() {
    let message = GeminiState::message("provider-a", "stored-model", parts()).unwrap();
    let saved = serde_json::to_value(&message).unwrap();
    let last = message.content.len() - 1;
    for invalid in [json!([]), json!(["missing"]), json!(["same", "same"])] {
        let mut changed = saved.clone();
        changed["content"][last]["state"]["call_ids"] = invalid;
        assert!(serde_json::from_value::<Message>(changed).is_err());
    }
    for invalid in [
        vec![],
        vec![json!(null)],
        vec![json!({"text":false})],
        vec![json!({"functionCall":{"name":"f","args":[]}})],
        vec![json!({"thoughtSignature":false})],
        vec![json!({"functionCall":{},"function_call":{}})],
        vec![json!({"text":"ambiguous","functionCall":{"name":"f","args":{}}})],
    ] {
        assert!(GeminiState::message("provider-a", "model", invalid).is_err());
    }
}

#[test]
fn native_image_parts_and_signature_only_thought_images_keep_their_projection() {
    let parts = vec![
        json!({"inlineData":{"mimeType":"image/png","data":"YQ=="},"thoughtSignature":"final-signature"}),
        json!({"inlineData":{"mimeType":"image/png","data":"Yg=="},"thought":true,"thoughtSignature":"hidden-thought-signature"}),
    ];
    let message = GeminiState::message("provider-a", "model", parts.clone()).unwrap();
    assert_eq!(message.content.len(), 2);
    assert!(
        matches!(&message.content[0],ContentBlock::Image {source} if source.data.as_ref() == "YQ==")
    );
    let ContentBlock::GeminiState { state } = message.content.last().unwrap() else {
        panic!("state missing")
    };
    assert_eq!(
        state.replay(&message, "provider-a", "model").unwrap(),
        parts
    );
}

#[test]
fn state_is_not_exposed_as_a_tool_or_transcript_and_foreign_wires_refuse_it() {
    let message = GeminiState::message(
        "provider-a",
        "model",
        vec![json!({"thoughtSignature":"opaque-only"})],
    )
    .unwrap();
    assert!(message.text().is_empty());
    assert!(message.transcript_display_text().is_empty());
    assert!(message.tool_calls().is_empty());
    assert!(refuse_foreign_state(&[message]).is_err());
    assert!(refuse_foreign_state(&[Message::user("plain")]).is_ok());
    let legacy: Message = serde_json::from_value(
        json!({"role":"assistant","content":[{"type":"text","text":"old"}]}),
    )
    .unwrap();
    assert_eq!(legacy.text(), "old");
}

#[test]
fn projection_identity_ignores_json_object_key_order_and_preserves_snake_case_parts() {
    let message = GeminiState::message("provider-a","model",vec![json!({"function_call":{"name":"lookup","args":{"a":1,"b":2}},"thought_signature":"opaque-snake"})]).unwrap();
    let ContentBlock::GeminiState { state } = message.content.last().unwrap() else {
        panic!("native state missing")
    };
    let mut reordered = message.clone();
    if let ContentBlock::ToolUse { input, .. } = &mut reordered.content[0] {
        *input = serde_json::from_str("{\"b\":2,\"a\":1}").unwrap();
    }
    assert_eq!(
        state.replay(&message, "provider-a", "model").unwrap(),
        state.replay(&reordered, "provider-a", "model").unwrap()
    );
}

#[test]
fn native_state_is_retained_for_replay_and_excluded_from_training_trajectories() {
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("trajectory.jsonl");
    let recorder = crate::rl_trajectory::RlTrajectoryRecorder::from_config(Some(
        crate::rl_trajectory::RlTrajectoryConfig::new(&path),
    ))
    .unwrap();
    let message = GeminiState::message("provider-a", "model", parts()).unwrap();
    let estimate = crate::llm::estimate_message_tokens(std::slice::from_ref(&message));
    let restored: Message = serde_json::from_slice(&serde_json::to_vec(&message).unwrap()).unwrap();
    assert_eq!(
        crate::llm::estimate_message_tokens(std::slice::from_ref(&restored)),
        estimate
    );
    assert!(estimate > crate::llm::estimate_message_tokens(&[Message::assistant("answer")]));
    recorder.record_llm_request("session-a", 1, &[message], None, &[], estimate);
    let text = std::fs::read_to_string(&path).unwrap();
    assert_eq!(text.lines().count(), 1);
    assert!(!text.contains("synthetic-private-signature"));
    assert!(!text.contains("synthetic-tool-signature"));
    assert!(!text.contains("gemini_state"));
    let record: serde_json::Value = serde_json::from_str(&text).unwrap();
    assert_eq!(
        record["payload"]["messages"][0]["content"]
            .as_array()
            .unwrap()
            .len(),
        3
    );
}

#[test]
fn native_agent_tool_recovery_preserves_examples_empty_parts_and_actual_calls() {
    let text = "Example: <tool_call>web_search query=\"x\" /></tool_call>";
    for native_call in [false, true] {
        let mut parts = vec![
            json!({"text":text}),
            json!({"text":"","thoughtSignature":"opaque-empty-part"}),
        ];
        if native_call {
            parts.push(json!({"functionCall":{"id":"actual-call","name":"lookup","args":{}},"thoughtSignature":"opaque-tool"}));
        }
        let mut message = GeminiState::message("provider-a", "model", parts.clone()).unwrap();
        let calls = crate::llm::recover_text_tool_calls(&mut message);
        assert_eq!(calls.len(), usize::from(native_call));
        assert_eq!(message.text(), text);
        if native_call {
            assert_eq!(calls[0].id, "actual-call");
            assert_eq!(calls[0].name, "lookup");
        }
        let ContentBlock::GeminiState { state } = message.content.last().unwrap() else {
            panic!("state missing")
        };
        assert_eq!(
            state.replay(&message, "provider-a", "model").unwrap(),
            parts
        );
    }
}
