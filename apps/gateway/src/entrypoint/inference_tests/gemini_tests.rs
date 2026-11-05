use super::*;
use crate::config::{InferenceProtocol, InferenceWire};
use crate::entrypoint::inference_usage_tests::{
    spawn_json_backend_with_owned_body, spawn_sse_backend_with_body, usage_state,
};
use serde_json::{json, Value};

fn config(backend: SocketAddr, key: &str) -> GatewayConfig {
    let mut config = inference_config(backend, key, Utc::now() + ChronoDuration::hours(1));
    for route in config.inference.as_mut().unwrap().routes.values_mut() {
        route.wire = InferenceWire::Openai;
        let model = route.models.remove("allowed-model").unwrap();
        route.models.insert("provider/allowed-model".into(), model);
        for grant in route.grants.values_mut() {
            grant.models = vec!["provider/allowed-model".into()];
            grant.endpoints = vec![
                InferenceEndpoint::GeminiModels,
                InferenceEndpoint::GeminiGenerate,
                InferenceEndpoint::GeminiCountTokens,
            ];
        }
        for model in route.models.values_mut() {
            model.targets[0].protocol = InferenceProtocol::Gemini;
        }
    }
    config.validate().unwrap();
    config
}

#[tokio::test]
async fn gemini_native_json_and_sse_reach_the_path_model_with_original_body() {
    for streaming in [false, true] {
        let output = json!({"candidates":[{"index":0,"content":{"role":"model","parts":[{"functionCall":{"name":"lookup","args":{"q":"中"}},"thoughtSignature":"opaque"}]},"finishReason":"STOP"}],"usageMetadata":{"promptTokenCount":10,"toolUsePromptTokenCount":2,"candidatesTokenCount":6,"thoughtsTokenCount":5,"cachedContentTokenCount":4,"totalTokenCount":23}});
        let wire = if streaming {
            format!("data: {output}\r\n\r\n")
        } else {
            output.to_string()
        };
        let (backend, captured) = if streaming {
            spawn_sse_backend_with_body(wire.clone()).await
        } else {
            spawn_json_backend_with_owned_body(wire.clone()).await
        };
        let key = inference_key('g');
        let mut config = config(backend, &key);
        for route in config.inference.as_mut().unwrap().routes.values_mut() {
            route
                .models
                .get_mut("provider/allowed-model")
                .unwrap()
                .targets[0]
                .upstream_model = "actual/中+id".into();
            for model in route.models.values_mut() {
                model.targets[0].api_key_env = Some("SUPPLIER_REQUIRED".into());
            }
        }
        let mut state = gateway_state(&config);
        Arc::get_mut(&mut state).unwrap().inference_authorizer =
            Some(Arc::new(InferenceAuthorizer::with_supplier_keys(
                config.inference.as_ref().unwrap(),
                None,
                &[crate::inference::SupplierCredential {
                    name: "model-service".into(),
                    api_key: "synthetic-provider-key".into(),
                }],
            )));
        let (address, shutdown, handle) = start_test_entrypoint(state).await;
        let method = if streaming {
            "streamGenerateContent"
        } else {
            "generateContent"
        };
        let input = r#"{ "contents":[{"role":"user","parts":[{"text":"hello","thoughtSignature":"opaque"}]}], "generationConfig":{"maxOutputTokens":20} }"#;
        let response = reqwest::Client::new().post(format!("http://{address}/v1beta/models/provider/allowed-model:{method}?key={key}&trace=one&trace=two"))
            .header("content-type", "application/json").body(input).send().await.unwrap();
        let status = response.status();
        let text = response.text().await.unwrap();
        assert_eq!(status, 200, "{text}");
        assert_eq!(text, wire);
        let raw = String::from_utf8(captured.await.unwrap()).unwrap();
        let query = if streaming {
            "trace=one&trace=two&alt=sse"
        } else {
            "trace=one&trace=two"
        };
        assert!(
            raw.starts_with(&format!(
                "POST /v1beta/models/actual%2F%E4%B8%AD+id:{method}?{query} HTTP/1.1\r\n"
            )),
            "{raw}"
        );
        assert!(!raw.contains(&key));
        assert!(raw
            .to_ascii_lowercase()
            .contains("x-goog-api-key: synthetic-provider-key\r\n"));
        assert!(!raw.to_ascii_lowercase().contains("authorization:"));
        assert_eq!(raw.split_once("\r\n\r\n").unwrap().1, input);
        stop_test_entrypoint(shutdown, handle).await;
    }
}

#[tokio::test]
async fn gemini_counter_has_an_independent_grant_and_never_creates_generation_spend() {
    let key = inference_key('h');
    let poison = json!({"totalTokens":37,"usageMetadata":{"promptTokenCount":5000,"candidatesTokenCount":4000,"totalTokenCount":9000}});
    let (backend, captured) = spawn_json_backend_with_owned_body(poison.to_string()).await;
    let mut config = config(backend, &key);
    for route in config.inference.as_mut().unwrap().routes.values_mut() {
        for grant in route.grants.values_mut() {
            grant.endpoints = vec![InferenceEndpoint::GeminiCountTokens];
            grant.limits.tokens_per_minute = 1;
        }
    }
    let directory = tempfile::tempdir().unwrap();
    let (state, spool) = usage_state(&config, directory.path(), 1024 * 1024).await;
    let (address, shutdown, handle) = start_test_entrypoint(state).await;
    let client = reqwest::Client::new();
    let response = client.post(format!("http://{address}/v1beta/models/provider%2Fallowed-model:countTokens"))
        .header("x-goog-api-key", &key).json(&json!({"contents":[{"parts":[{"text":"long enough to exceed generation token quota"}]}]})).send().await.unwrap();
    assert_eq!(response.status(), 200);
    assert_eq!(response.headers()["x-aster-token-count-source"], "upstream");
    assert_eq!(response.json::<Value>().await.unwrap(), poison);
    let raw = String::from_utf8(captured.await.unwrap()).unwrap();
    assert!(raw.starts_with("POST /v1beta/models/internal-allowed-model:countTokens HTTP/1.1\r\n"));
    let denied = client
        .post(format!(
            "http://{address}/v1beta/models/provider/allowed-model:generateContent"
        ))
        .header("x-goog-api-key", &key)
        .json(&json!({"contents":[]}))
        .send()
        .await
        .unwrap();
    assert_eq!(denied.status(), 403);
    stop_test_entrypoint(shutdown, handle).await;
    assert_eq!(spool.status().retained_records, 0);
    spool.shutdown().await;
}

#[tokio::test]
async fn gemini_discovery_filters_grants_and_uses_google_errors() {
    let key = inference_key('i');
    let (backend, _) = spawn_capturing_backend().await;
    let config = config(backend, &key);
    let (address, shutdown, handle) = start_test_entrypoint(gateway_state(&config)).await;
    let client = reqwest::Client::new();
    let response = client
        .get(format!("http://{address}/v1beta/models?pageSize=1"))
        .header("x-goog-api-key", &key)
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 200);
    let body: Value = response.json().await.unwrap();
    assert_eq!(body["models"].as_array().unwrap().len(), 1);
    assert_eq!(body["models"][0]["name"], "models/provider/allowed-model");
    for (url, token, status, error) in [
        (
            "/v1beta/models?pageSize=bad",
            key.as_str(),
            400,
            "INVALID_ARGUMENT",
        ),
        ("/v1beta/models", "wrong-key", 401, "UNAUTHENTICATED"),
    ] {
        let response = client
            .get(format!("http://{address}{url}"))
            .header("x-goog-api-key", token)
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), status);
        let body: Value = response.json().await.unwrap();
        assert_eq!(body["error"]["code"], status);
        assert_eq!(body["error"]["status"], error);
    }
    stop_test_entrypoint(shutdown, handle).await;
}

#[tokio::test]
async fn gemini_unknown_methods_and_conflicting_keys_never_reach_upstream() {
    let key = inference_key('j');
    let (backend, mut captured) = spawn_capturing_backend().await;
    let config = config(backend, &key);
    let (address, shutdown, handle) = start_test_entrypoint(gateway_state(&config)).await;
    let client = reqwest::Client::new();
    for method in ["unknown", "embedContent"] {
        let response = client
            .post(format!(
                "http://{address}/v1beta/models/provider/allowed-model:{method}"
            ))
            .header("x-goog-api-key", &key)
            .json(&json!({}))
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), 404);
        assert_eq!(
            response.json::<Value>().await.unwrap()["error"]["status"],
            "NOT_FOUND"
        );
    }
    let response = client
        .post(format!(
            "http://{address}/v1beta/models/provider/allowed-model:generateContent?key=other"
        ))
        .header("x-goog-api-key", &key)
        .json(&json!({}))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 401);
    assert!(
        tokio::time::timeout(Duration::from_millis(100), &mut captured)
            .await
            .is_err()
    );
    stop_test_entrypoint(shutdown, handle).await;
}

#[tokio::test]
async fn gemini_generation_refuses_a_cloud_spool_that_cannot_decode_its_endpoint() {
    let key = inference_key('k');
    let (backend, mut captured) = spawn_capturing_backend().await;
    let config = config(backend, &key);
    let directory = tempfile::tempdir().unwrap();
    let (state, spool) = usage_state(&config, directory.path(), 1024 * 1024).await;
    let (address, shutdown, handle) = start_test_entrypoint(state).await;
    for (token, expected) in [(key.as_str(), 503), ("wrong-key", 401)] {
        let response = reqwest::Client::new()
            .post(format!(
                "http://{address}/v1beta/models/provider/allowed-model:generateContent"
            ))
            .header("x-goog-api-key", token)
            .json(&json!({"contents":[]}))
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), expected);
    }
    assert!(
        tokio::time::timeout(Duration::from_millis(100), &mut captured)
            .await
            .is_err()
    );
    stop_test_entrypoint(shutdown, handle).await;
    assert_eq!(spool.status().retained_records, 0);
    spool.shutdown().await;
}

#[tokio::test]
#[ignore = "requires Node and the installed Studio @google/genai dependency"]
async fn gemini_official_sdk_exercises_the_real_gateway_listener() {
    use std::process::Stdio;
    use tokio::io::{AsyncBufReadExt, BufReader};

    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap();
    let script = root.join("apps/studio/tests/e2e/local-gateway/gemini-sdk-fixture.mjs");
    let mut child = tokio::process::Command::new("node")
        .arg(script)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true)
        .spawn()
        .unwrap();
    let mut stdout = BufReader::new(child.stdout.take().unwrap());
    let mut ready = String::new();
    tokio::time::timeout(Duration::from_secs(30), stdout.read_line(&mut ready))
        .await
        .unwrap()
        .unwrap();
    let ready: Value = serde_json::from_str(&ready).unwrap();
    let port = u16::try_from(ready["port"].as_u64().unwrap()).unwrap();
    let key = inference_key('a');
    let mut config = config(SocketAddr::from(([127, 0, 0, 1], port)), &key);
    for route in config.inference.as_mut().unwrap().routes.values_mut() {
        route
            .models
            .get_mut("provider/allowed-model")
            .unwrap()
            .targets[0]
            .upstream_model = "sdk-upstream".into();
        for model in route.models.values_mut() {
            model.targets[0].api_key_env = Some("SUPPLIER_REQUIRED".into());
        }
        for grant in route.grants.values_mut() {
            grant.limits.request_burst = 10;
        }
    }
    let mut state = gateway_state(&config);
    Arc::get_mut(&mut state).unwrap().inference_authorizer =
        Some(Arc::new(InferenceAuthorizer::with_supplier_keys(
            config.inference.as_ref().unwrap(),
            None,
            &[crate::inference::SupplierCredential {
                name: "model-service".into(),
                api_key: "fixture-provider-key".into(),
            }],
        )));
    let (address, shutdown, handle) = start_test_entrypoint(state).await;
    let frame = json!({"baseUrl":format!("http://{address}"),"apiKey":key,"alias":"provider/allowed-model","upstreamModel":"sdk-upstream","providerKey":"fixture-provider-key"});
    let result = tokio::time::timeout(Duration::from_secs(30), async {
        let mut stdin = child.stdin.take().unwrap();
        stdin
            .write_all(format!("{frame}\n").as_bytes())
            .await
            .unwrap();
        drop(stdin);
        child.wait_with_output().await
    })
    .await;
    stop_test_entrypoint(shutdown, handle).await;
    let output = result
        .expect("SDK fixture completed within its fixed deadline")
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let mut report = String::new();
    stdout.read_to_string(&mut report).await.unwrap();
    let report: Value = serde_json::from_str(&report).unwrap();
    assert_eq!(report["totalTokens"], 37);
    assert_eq!(report["streamChunks"], 2);
    assert_eq!(report["rejected"], json!([401, 403]));
    println!("GOOGLE_SDK_REPORT={report}");
}
