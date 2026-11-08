use super::*;

#[test]
fn exact_native_clocks_and_execution_counters_stay_inside_powers_private_timing_boundary() {
    use crate::execution::{XqGeneratedTokens, XqGenerationStats, XqGenerationTimings};
    let generated = XqGeneratedTokens {
        tokens: vec![20, 21],
        prompt_tokens: 4,
        processed_tokens: 5,
        pending_tail: 21,
        stop_reason: XqStopReason::Length,
        stats: XqGenerationStats {
            model_steps: 3,
            executed_layers_by_ordinal: [60, 60, 0, 0, 0, 0, 0, 0],
            ..Default::default()
        },
        timings: XqGenerationTimings {
            first_token_ns: 100,
            last_token_ns: 200,
            finished_ns: 250,
        },
    };
    let mut value = native_metadata(&generated, 4, 2);
    assert_eq!(
        value["usage"],
        serde_json::json!({"prompt_tokens":4,"completion_tokens":2,"total_tokens":6})
    );
    assert_eq!(
        value["timings"]["native_generation"]["timings"]["lastTokenNs"],
        200
    );
    assert_eq!(value["timings"]["native_generation"]["replica_index"], 2);
    assert_eq!(
        value["timings"]["native_generation"]["readonly_kernel_input_policy"],
        if cfg!(feature = "xq-borrowed-kernel-inputs") {
            "borrow-contiguous-copy-strided-v1"
        } else {
            "force-copy-v1"
        }
    );
    assert!(value.get("native_generation").is_none());
    // The unchanged Power presentation owner removes this exact top-level
    // key in suppress mode; no second root key can retain the exact counters.
    value.as_object_mut().unwrap().remove("timings");
    assert_eq!(
        value
            .as_object()
            .unwrap()
            .keys()
            .map(String::as_str)
            .collect::<Vec<_>>(),
        ["source", "usage"]
    );
}

#[test]
fn native_greedy_options_accept_explicit_supported_controls_and_refuse_policy_loss() {
    let base = serde_json::json!({"messages":[{"role":"user","content":"你好"}],"temperature":0,"top_p":1,"num_parallel":1,"stream_options":{"include_usage":true}});
    let request: ChatRequest = serde_json::from_value(base.clone()).unwrap();
    validate_chat_options(&request, 256).unwrap();
    for (field, value) in [
        ("temperature", serde_json::json!(0.7)),
        ("stop", serde_json::json!(["end"])),
        ("num_ctx", serde_json::json!(257)),
        ("session_id", serde_json::json!("another-owner")),
        ("tools", serde_json::json!([])),
        ("response_format", serde_json::json!({"type":"json_object"})),
    ] {
        let mut changed = base.clone();
        changed[field] = value;
        let request: ChatRequest = serde_json::from_value(changed).unwrap();
        assert!(validate_chat_options(&request, 256).is_err(), "{field}");
    }
}
