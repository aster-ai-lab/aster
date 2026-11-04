#![cfg(test)]

//! Safe discovery for the long-horizon campaign. Parsing an explicit ACL does
//! not instantiate an Agent, import account state, or contact a provider.

use aster_core::config::{CodeConfig, StorageBackend};
use serde_json::{json, Value};
use std::path::PathBuf;

fn summary(config: &CodeConfig) -> Value {
    json!({
        "default_model": config.default_model,
        "models": config.list_models().into_iter()
            .map(|(provider, model)| format!("{}/{}", provider.name, model.id))
            .collect::<Vec<_>>(),
        "protocols": config.providers.iter()
            .map(|provider| (provider.name.clone(), provider.resolved_protocol().as_str()))
            .collect::<std::collections::BTreeMap<_, _>>(),
        "explicit_account_imports": config.import_accounts.iter()
            .map(|account| account.as_str()).collect::<Vec<_>>(),
        "persistent_state_overrides": {
            "sessions_dir": config.sessions_dir.is_some(),
            "memory_dir": config.memory_dir.is_some(),
            "external_store": config.storage_backend == StorageBackend::Custom,
            "skill_dirs": !config.skill_dirs.is_empty(),
            "agent_dirs": !config.agent_dirs.is_empty(),
        },
        "provider_requests": 0,
        "agent_created": false,
    })
}

fn safe_failure_details(error: &anyhow::Error) -> Value {
    let io = error
        .chain()
        .find_map(|cause| cause.downcast_ref::<std::io::Error>());
    let retry = error.downcast_ref::<aster_core::llm::RetryExhaustedError>();
    let terminal = error.downcast_ref::<aster_core::llm::NonRetryableLlmError>();
    let core = error.downcast_ref::<aster_core::error::CodeError>();
    json!({
        "connection_refused": io.is_some_and(|error| error.kind() == std::io::ErrorKind::ConnectionRefused),
        "io_kind": io.map(|error| format!("{:?}", error.kind())),
        "retry_attempts": retry.map(aster_core::llm::RetryExhaustedError::attempts),
        "retry_status": retry.map(aster_core::llm::RetryExhaustedError::status_code),
        "terminal_status": terminal.and_then(aster_core::llm::NonRetryableLlmError::status),
        "core_code": core.map(aster_core::error::CodeError::runtime_code),
    })
}

#[test]
fn failure_diagnostics_preserve_categories_without_exporting_payloads() {
    let error = anyhow::Error::new(aster_core::llm::NonRetryableLlmError::from_status(
        "fixture",
        401,
        "synthetic-secret",
    ))
    .context("private synthetic-secret context");
    let details = safe_failure_details(&error);
    assert_eq!(details["terminal_status"], 401);
    assert!(!details.to_string().contains("synthetic-secret"));
    let refused = safe_failure_details(
        &std::io::Error::new(std::io::ErrorKind::ConnectionRefused, "synthetic-secret").into(),
    );
    assert_eq!(refused["connection_refused"], true);
    assert!(!refused.to_string().contains("synthetic-secret"));
    assert!(
        safe_failure_details(&anyhow::anyhow!("synthetic-secret"))["terminal_status"].is_null()
    );
}

#[test]
fn discovery_exports_catalog_without_provider_secrets() {
    let config = CodeConfig::from_acl(
        r#"
default_model = "fixture/model"
providers "fixture" {
  api_key = "fixture-secret-must-never-be-exported"
  base_url = "https://fixture.invalid/private-upstream"
  models "model" {}
}
"#,
    )
    .unwrap();
    let report = summary(&config);
    assert_eq!(report["default_model"], "fixture/model");
    assert_eq!(report["models"], json!(["fixture/model"]));
    assert_eq!(report["provider_requests"], 0);
    let serialized = report.to_string();
    assert!(!serialized.contains("fixture-secret"));
    assert!(!serialized.contains("private-upstream"));
    assert!(!serialized.contains("api_key"));
}

#[test]
fn discovery_flags_state_overrides_without_exporting_their_contents() {
    let mut config = CodeConfig::default();
    config.memory_dir = Some(PathBuf::from("private-memory-sentinel"));
    config.sessions_dir = Some(PathBuf::from("private-session-sentinel"));
    config.storage_backend = StorageBackend::Custom;
    config.storage_url = Some("private-connection-sentinel".into());
    let report = summary(&config);
    let flags = &report["persistent_state_overrides"];
    assert_eq!(flags["memory_dir"], true);
    assert_eq!(flags["sessions_dir"], true);
    assert_eq!(flags["external_store"], true);
    assert!(!report.to_string().contains("sentinel"));
}

#[test]
#[ignore = "reads only the explicitly selected live ACL; never sends a model request"]
fn inspect_live_acl_without_activating_user_state() {
    let path = std::env::var_os("ASTER_CONFIG_FILE")
        .map(PathBuf::from)
        .expect("ASTER_CONFIG_FILE must explicitly select the trusted ACL");
    let config = CodeConfig::from_file(&path).unwrap_or_else(|_| {
        panic!("selected ACL could not be parsed; source/credentials withheld")
    });
    println!("LONG_HORIZON_PREFLIGHT {}", summary(&config));
}

#[tokio::test]
#[ignore = "one bounded real-provider health probe; not ACP E2E acceptance"]
async fn selected_real_provider_health_without_account_or_session_activation() {
    use aster_core::llm::{create_client_with_config, Message, NonRetryableLlmError};
    use std::time::{Duration, Instant};

    let path = std::env::var_os("ASTER_CONFIG_FILE")
        .map(PathBuf::from)
        .expect("ASTER_CONFIG_FILE must explicitly select the trusted ACL");
    let selected = std::env::var("ASTER_LIVE_MODEL")
        .expect("ASTER_LIVE_MODEL must select one exact declared provider/model");
    let (provider, model) = selected
        .split_once('/')
        .filter(|(provider, model)| !provider.is_empty() && !model.is_empty())
        .expect("ASTER_LIVE_MODEL must be provider/model");
    let config = CodeConfig::from_file(&path)
        .unwrap_or_else(|_| panic!("selected ACL could not be parsed; credentials withheld"));
    let mut llm = config
        .llm_config(provider, model)
        .expect("selected provider/model is not explicitly configured");
    llm.max_tokens = Some(512);
    let mut retry = llm.retry_config.take().unwrap_or_default();
    retry.max_retries = 0;
    llm.retry_config = Some(retry);
    let client = create_client_with_config(llm);
    let started = Instant::now();
    let result = tokio::time::timeout(
        Duration::from_secs(120),
        client.complete(
            &[Message::user("Reply with exactly pong. Do not use tools.")],
            None,
            &[],
        ),
    )
    .await;
    let mut report = json!({
        "model": selected,
        "phase": "provider_health_only_not_acp_e2e",
        "generation_calls": 1,
        "retry_limit": 0,
        "output_token_limit": 512,
        "deadline_ms": 120_000,
        "elapsed_ms": started.elapsed().as_millis(),
        "agent_created": false,
        "account_imports_activated": false,
    });
    match result {
        Ok(Ok(response)) => {
            report["protocol_valid"] = true.into();
            report["tiny_probe_correct"] =
                response.text().trim().eq_ignore_ascii_case("pong").into();
            report["adapter_usage"] = serde_json::to_value(response.usage).unwrap();
            report["raw_usage_presence_verified"] = false.into();
            println!("LONG_HORIZON_PROVIDER_HEALTH {report}");
            assert_eq!(
                report["tiny_probe_correct"], true,
                "valid response did not satisfy the tiny probe"
            );
        }
        Ok(Err(error)) => {
            report["protocol_valid"] = false.into();
            report["failure_details"] = safe_failure_details(&error);
            report["terminal_http_status"] = error
                .downcast_ref::<NonRetryableLlmError>()
                .and_then(NonRetryableLlmError::status)
                .map_or(Value::Null, |status| status.into());
            report["attribution"] = "undetermined".into();
            println!("LONG_HORIZON_PROVIDER_HEALTH {report}");
            panic!("provider health probe failed; raw error/credentials not exported");
        }
        Err(_) => {
            report["protocol_valid"] = false.into();
            report["timeout"] = true.into();
            println!("LONG_HORIZON_PROVIDER_HEALTH {report}");
            panic!("provider health probe exceeded its deadline");
        }
    }
}

#[path = "long_horizon/upstream.rs"]
mod upstream;

#[tokio::test]
#[ignore = "one bounded native provider usage probe; explicit trusted config/model only; no Agent or tools"]
async fn inspect_native_stream_usage_without_exporting_response_text() {
    let path =
        PathBuf::from(std::env::var_os("ASTER_CONFIG_FILE").expect("explicit trusted config"));
    let selected = std::env::var("ASTER_LIVE_MODEL").expect("explicit model");
    let (provider, model) = selected.split_once('/').expect("provider/model");
    let config = CodeConfig::from_file(&path).unwrap_or_else(|_| panic!("trusted config refused"));
    let route =
        upstream::provider_route(config.llm_config(provider, model).expect("declared model"))
            .await
            .unwrap_or_else(|_| panic!("native route refused"));
    assert_eq!(route.protocol, upstream::Protocol::Anthropic);
    use aster_core::llm::{http::ReqwestHttpClient, HttpClient};
    let client = ReqwestHttpClient::without_redirects().expect("bounded probe client");
    let headers = route
        .headers
        .iter()
        .map(|(name, value)| (name.as_str(), value.as_str()))
        .collect();
    let body = json!({"model":route.model,"max_tokens":64,"stream":true,
        "messages":[{"role":"user","content":"Reply PONG."}]});
    let bytes = tokio::time::timeout(std::time::Duration::from_secs(120), async {
        let mut response = client
            .post_streaming(
                &route.endpoint,
                headers,
                &body,
                tokio_util::sync::CancellationToken::new(),
            )
            .await
            .unwrap_or_else(|_| panic!("native probe transport failed"));
        assert_eq!(response.status, 200, "probe upstream status");
        let mut bytes = Vec::new();
        while let Some(chunk) =
            std::future::poll_fn(|cx| response.byte_stream.as_mut().poll_next(cx)).await
        {
            let chunk = chunk.unwrap_or_else(|_| panic!("native probe body failed"));
            assert!(bytes.len() + chunk.len() <= 65536, "probe body bound");
            bytes.extend_from_slice(&chunk);
        }
        bytes
    })
    .await
    .expect("native probe deadline");
    let source = std::str::from_utf8(&bytes).expect("native SSE UTF-8");
    let mut counters = Vec::new();
    for data in source.lines().filter_map(|line| line.strip_prefix("data:")) {
        let Ok(event) = serde_json::from_str::<Value>(data.trim()) else {
            continue;
        };
        let usage = event
            .get("usage")
            .or_else(|| event.pointer("/message/usage"));
        if let Some(usage) = usage {
            let reported: serde_json::Map<String, Value> = [
                "input_tokens",
                "output_tokens",
                "cache_read_input_tokens",
                "cache_creation_input_tokens",
                "total_tokens",
            ]
            .into_iter()
            .filter_map(|key| {
                usage
                    .get(key)
                    .filter(|v| v.is_u64())
                    .map(|v| (key.into(), v.clone()))
            })
            .collect();
            counters
                .push(json!({"event":event.get("type").and_then(Value::as_str),"usage":reported}));
        }
    }
    assert!(!counters.is_empty(), "provider must report usage");
    println!(
        "NATIVE_STREAM_USAGE {}",
        json!({"model":selected,"requests":1,"output_limit":64,"events":counters,"response_text_exported":false})
    );
}
