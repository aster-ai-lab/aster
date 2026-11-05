use super::*;
use crate::config::{InferenceProtocol, InferenceWire};
use crate::entrypoint::inference_usage_tests::{
    lifecycle_events, spawn_json_backend_with_owned_body, spawn_sse_backend_with_body, usage_state,
};
use serde_json::{json, Value};

#[tokio::test]
async fn messages_native_json_and_sse_preserve_system_tools_thinking_and_usage() {
    let native = json!({"id":"msg_1","type":"message","role":"assistant","model":"served-model",
        "content":[{"type":"thinking","thinking":"why","signature":"opaque-signature"},{"type":"tool_use","id":"tool_1","name":"lookup","input":{"q":"中"}}],
        "stop_reason":"tool_use","stop_sequence":null,"usage":{"input_tokens":10,"cache_creation_input_tokens":2,"cache_read_input_tokens":4,"output_tokens":6}});
    for streaming in [false, true] {
        let wire = if streaming {
            [
                json!({"type":"message_start","message":{"id":"msg_1","type":"message","role":"assistant","model":"served-model","content":[],"usage":{"input_tokens":10,"cache_creation_input_tokens":2,"cache_read_input_tokens":4,"output_tokens":0}}}),
                json!({"type":"content_block_start","index":0,"content_block":{"type":"thinking","thinking":"","signature":""}}),
                json!({"type":"content_block_delta","index":0,"delta":{"type":"thinking_delta","thinking":"why"}}),
                json!({"type":"content_block_delta","index":0,"delta":{"type":"signature_delta","signature":"opaque-signature"}}),
                json!({"type":"content_block_stop","index":0}),
                json!({"type":"content_block_start","index":1,"content_block":{"type":"tool_use","id":"tool_1","name":"lookup","input":{}}}),
                json!({"type":"content_block_delta","index":1,"delta":{"type":"input_json_delta","partial_json":"{\"q\":\"中\"}"}}),
                json!({"type":"content_block_stop","index":1}),
                json!({"type":"message_delta","delta":{"stop_reason":"tool_use","stop_sequence":null},"usage":{"output_tokens":6}}),
                json!({"type":"message_stop"}),
            ].iter().map(|event|format!("event: {}\ndata: {event}\n\n",event["type"].as_str().unwrap())).collect()
        } else {
            native.to_string()
        };
        let (backend, captured) = if streaming {
            spawn_sse_backend_with_body(wire.clone()).await
        } else {
            spawn_json_backend_with_owned_body(wire.clone()).await
        };
        let key = inference_key('a');
        let mut config = inference_config(backend, &key, Utc::now() + ChronoDuration::hours(1));
        for route in config.inference.as_mut().unwrap().routes.values_mut() {
            route.wire = InferenceWire::Openai;
            for grant in route.grants.values_mut() {
                grant.endpoints.push(InferenceEndpoint::Messages);
            }
            for model in route.models.values_mut() {
                model.targets[0].protocol = InferenceProtocol::Anthropic;
            }
        }
        let directory = tempfile::tempdir().unwrap();
        let (state, spool) = usage_state(&config, directory.path(), 1024 * 1024).await;
        let (address, shutdown, handle) = start_test_entrypoint(state).await;
        let input = json!({"model":"allowed-model","system":[{"type":"text","text":"instructions","cache_control":{"type":"ephemeral"}}],"messages":[{"role":"user","content":[{"type":"text","text":"hello"}]}],"max_tokens":20,"stream":streaming});
        let response = reqwest::Client::new()
            .post(format!("http://{address}/v1/messages"))
            .header("x-api-key", &key)
            .header("anthropic-version", "2023-06-01")
            .header("anthropic-beta", "fixture-beta")
            .json(&input)
            .send()
            .await
            .unwrap();
        let status = response.status();
        let text = response.text().await.unwrap();
        assert_eq!(status, 200, "{text}");
        assert_eq!(text, wire);
        let raw = String::from_utf8(captured.await.unwrap()).unwrap();
        assert!(raw.starts_with("POST /v1/messages HTTP/1.1\r\n"));
        assert!(!raw.contains(&key));
        assert!(raw
            .to_ascii_lowercase()
            .contains("anthropic-beta: fixture-beta"));
        let forwarded: Value = serde_json::from_str(raw.split_once("\r\n\r\n").unwrap().1).unwrap();
        let mut expected = input;
        expected["model"] = json!("internal-allowed-model");
        assert_eq!(forwarded, expected);
        let events = lifecycle_events(&spool, 4).await;
        let terminal = events.last().unwrap();
        assert_eq!(terminal["total_tokens"], 22);
        assert_eq!(terminal["tokens"]["cached_input_tokens"], 4);
        stop_test_entrypoint(shutdown, handle).await;
        spool.shutdown().await;
    }
}

#[tokio::test]
async fn count_tokens_has_its_own_grant_and_does_not_charge_generation_usage() {
    let key = inference_key('a');
    let poison = json!({"input_tokens":37,"usage":{"input_tokens":5000,"output_tokens":4000}});
    let (backend, captured) = spawn_json_backend_with_owned_body(poison.to_string()).await;
    let mut config = inference_config(backend, &key, Utc::now() + ChronoDuration::hours(1));
    for route in config.inference.as_mut().unwrap().routes.values_mut() {
        route.wire = InferenceWire::Openai;
        for grant in route.grants.values_mut() {
            grant.endpoints.push(InferenceEndpoint::MessagesCountTokens);
            grant.limits.tokens_per_minute = 1;
        }
        for model in route.models.values_mut() {
            model.targets[0].protocol = InferenceProtocol::Anthropic;
        }
    }
    config.validate().unwrap();
    let directory = tempfile::tempdir().unwrap();
    let (state, spool) = usage_state(&config, directory.path(), 1024 * 1024).await;
    let (address, shutdown, handle) = start_test_entrypoint(state).await;
    let client = reqwest::Client::new();
    let denied = client
        .post(format!("http://{address}/v1/messages"))
        .header("x-api-key", &key)
        .json(&json!({"model":"allowed-model","max_tokens":20,"messages":[]}))
        .send()
        .await
        .unwrap();
    assert_eq!(denied.status(), 403);
    let response=client.post(format!("http://{address}/v1/messages/count_tokens")).header("x-api-key",&key)
        .json(&json!({"model":"allowed-model","messages":[{"role":"user","content":"long enough to exceed the generation token quota"}]})).send().await.unwrap();
    assert_eq!(response.status(), 200);
    assert_eq!(response.json::<Value>().await.unwrap(), poison);
    let raw = String::from_utf8(captured.await.unwrap()).unwrap();
    assert!(raw.starts_with("POST /v1/messages/count_tokens HTTP/1.1\r\n"));
    stop_test_entrypoint(shutdown, handle).await;
    assert_eq!(
        spool.status().retained_records,
        0,
        "counter traffic must not enter the generation ledger contract"
    );
    spool.shutdown().await;
}
