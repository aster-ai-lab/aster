use super::*;
use crate::config::InferenceWire;
use crate::entrypoint::inference_usage_tests::{
    lifecycle_events, spawn_json_backend_with_owned_body, spawn_sse_backend_with_body, usage_state,
};
use serde_json::{json, Value};

#[tokio::test]
async fn responses_auth_models_revocation_and_quota_refuse_before_upstream() {
    for (case, status) in [
        ("authentication", 401),
        ("hidden", 403),
        ("missing", 403),
        ("revoked", 401),
        ("quota", 429),
    ] {
        let key = inference_key('r');
        let (backend, captured) = spawn_capturing_backend().await;
        let mut config = inference_config(backend, &key, Utc::now() + ChronoDuration::hours(1));
        if case == "revoked" {
            for credential in config.inference.as_mut().unwrap().credentials.values_mut() {
                credential.revoked = true;
            }
            for route in config.inference.as_mut().unwrap().routes.values_mut() {
                route.grants.clear();
            }
        }
        if case == "quota" {
            for route in config.inference.as_mut().unwrap().routes.values_mut() {
                for grant in route.grants.values_mut() {
                    grant.limits.tokens_per_minute = 1;
                }
            }
        }
        config.validate().unwrap();
        let (address, shutdown, handle) = start_test_entrypoint(gateway_state(&config)).await;
        let model = match case {
            "hidden" => "hidden-model",
            "missing" => "missing-model",
            _ => "allowed-model",
        };
        let bearer = if case == "authentication" {
            inference_key('x')
        } else {
            key.clone()
        };
        let response = reqwest::Client::new()
            .post(format!("http://{address}/v1/responses"))
            .bearer_auth(bearer)
            .json(&json!({"model":model,"input":"hello","max_output_tokens":1}))
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), status, "{case}");
        let body: Value = response.json().await.unwrap();
        assert!(!body.to_string().contains("hidden-model"));
        assert!(!body.to_string().contains("missing-model"));
        assert!(
            tokio::time::timeout(Duration::from_millis(100), captured)
                .await
                .is_err(),
            "{case} reached upstream"
        );
        stop_test_entrypoint(shutdown, handle).await;
    }
}

#[tokio::test]
async fn responses_native_and_translated_json_and_sse_preserve_tools_and_usage() {
    let usage = json!({"input_tokens":10,"output_tokens":6,"total_tokens":16,
        "input_tokens_details":{"cached_tokens":4},"output_tokens_details":{"reasoning_tokens":2}});
    let output = json!([
        {"type":"reasoning","id":"rs_0","content":[{"type":"reasoning_text","text":"why"}]},
        {"type":"function_call","id":"fc_0","call_id":"call_0","name":"lookup","arguments":"{\"q\":\"中\"}"}
    ]);
    let native = json!({"id":"resp-native","object":"response","status":"completed","model":"served-model","output":output,"usage":usage});
    let chat_usage = json!({"prompt_tokens":10,"completion_tokens":6,"total_tokens":16,
        "prompt_tokens_details":{"cached_tokens":4},"completion_tokens_details":{"reasoning_tokens":2}});
    let tool = json!({"id":"call_0","type":"function","function":{"name":"lookup","arguments":"{\"q\":\"中\"}"}});
    for translated in [false, true] {
        for streaming in [false, true] {
            let wire = match (translated, streaming) {
                (false, false) => native.to_string(),
                (false, true) => [
                    json!({"type":"response.output_item.added","output_index":1,"item":output[1]}),
                    json!({"type":"response.reasoning_text.delta","output_index":0,"delta":"why"}),
                    json!({"type":"response.function_call_arguments.delta","item_id":"fc_0","output_index":1,"delta":"{\"q\":\"中\"}"}),
                    json!({"type":"response.completed","response":native}),
                ].iter().map(|event| format!("event: {}\ndata: {event}\n\n",event["type"].as_str().unwrap())).collect(),
                (true, false) => json!({"id":"chat-1","object":"chat.completion","model":"served-model",
                    "choices":[{"index":0,"message":{"role":"assistant","content":null,"reasoning_content":"why","tool_calls":[tool]},"finish_reason":"tool_calls"}],"usage":chat_usage}).to_string(),
                (true, true) => {
                    let mut indexed_tool = tool.clone();
                    indexed_tool["index"] = json!(0);
                    [
                        json!({"id":"chat-1","model":"served-model","choices":[{"index":0,"delta":{"role":"assistant","reasoning_content":"why","tool_calls":[indexed_tool]},"finish_reason":null}]}),
                        json!({"id":"chat-1","model":"served-model","choices":[{"index":0,"delta":{},"finish_reason":"tool_calls"}]}),
                        json!({"choices":[],"usage":chat_usage}),
                    ].iter().map(|event|format!("data: {event}\n\n")).collect::<String>() + "data: [DONE]\n\n"
                }
            };
            let key = inference_key('r');
            let (backend, captured) = if streaming {
                spawn_sse_backend_with_body(wire.clone()).await
            } else {
                spawn_json_backend_with_owned_body(wire.clone()).await
            };
            let mut config = inference_config(backend, &key, Utc::now() + ChronoDuration::hours(1));
            for route in config.inference.as_mut().unwrap().routes.values_mut() {
                route.wire = InferenceWire::Openai;
                for model in route.models.values_mut() {
                    model.targets[0].translate = translated;
                }
            }
            config.validate().unwrap();
            let directory = tempfile::tempdir().unwrap();
            let (state, spool) = usage_state(&config, directory.path(), 1024 * 1024).await;
            let (address, shutdown, handle) = start_test_entrypoint(state).await;
            let mut input = json!({"model":"allowed-model","input":"prompt-stays-out-of-ledger","stream":streaming,"max_output_tokens":16});
            if !translated {
                input["previous_response_id"] = json!("resp-prior");
            }
            let response = reqwest::Client::new()
                .post(format!("http://{address}/v1/responses"))
                .bearer_auth(&key)
                .json(&input)
                .send()
                .await
                .unwrap();
            let status = response.status();
            let body = response.text().await.unwrap();
            assert_eq!(
                status, 200,
                "translated={translated},streaming={streaming}: {body}"
            );
            if !translated {
                assert_eq!(body, wire);
            } else {
                let value: Value = if streaming {
                    let events: Vec<Value> = body
                        .lines()
                        .filter_map(|line| line.strip_prefix("data: "))
                        .filter_map(|data| serde_json::from_str(data).ok())
                        .collect();
                    assert!(events
                        .iter()
                        .any(|event| event["type"] == "response.function_call_arguments.delta"));
                    assert!(events
                        .iter()
                        .any(|event| event["type"] == "response.reasoning_text.delta"));
                    assert_eq!(events.last().unwrap()["type"], "response.completed");
                    events.last().unwrap()["response"].clone()
                } else {
                    serde_json::from_str(&body).unwrap()
                };
                assert_eq!(value["status"], "completed");
                let items = value["output"].as_array().unwrap();
                let call = items
                    .iter()
                    .find(|item| item["type"] == "function_call")
                    .unwrap();
                assert_eq!(call["call_id"], "call_0");
                assert_eq!(call["name"], "lookup");
                assert_eq!(call["arguments"], "{\"q\":\"中\"}");
                assert!(items.iter().any(|item| item["type"] == "reasoning"));
                assert_eq!(value["usage"], usage);
            }
            let raw = String::from_utf8(captured.await.unwrap()).unwrap();
            let expected_path = if translated {
                "/v1/chat/completions"
            } else {
                "/v1/responses"
            };
            assert!(
                raw.starts_with(&format!("POST {expected_path} HTTP/1.1\r\n")),
                "{raw}"
            );
            assert!(!raw.contains(&key));
            let forwarded: Value =
                serde_json::from_str(raw.split_once("\r\n\r\n").unwrap().1).unwrap();
            assert_eq!(forwarded["model"], "internal-allowed-model");
            if translated {
                assert_eq!(forwarded["messages"][0]["content"], input["input"]);
                assert!(forwarded.get("input").is_none());
            } else {
                input["model"] = json!("internal-allowed-model");
                assert_eq!(forwarded, input);
            }
            let events = lifecycle_events(&spool, 4).await;
            let terminal = events.last().unwrap();
            assert_eq!(terminal["total_tokens"], 16);
            assert_eq!(terminal["tokens"]["prompt_tokens"], 10);
            assert_eq!(terminal["tokens"]["completion_tokens"], 6);
            assert_eq!(terminal["tokens"]["cached_input_tokens"], 4);
            assert_eq!(terminal["tokens"]["reasoning_tokens"], 2);
            let serialized = serde_json::to_string(&events).unwrap();
            assert!(!serialized.contains("prompt-stays-out-of-ledger"));
            assert!(!serialized.contains(&key));
            stop_test_entrypoint(shutdown, handle).await;
            spool.shutdown().await;
        }
    }
}
