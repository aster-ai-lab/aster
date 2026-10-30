#![cfg(test)]

//! Safe discovery for the long-horizon campaign. Parsing an explicit ACL does
//! not instantiate an Agent, import account state, or contact a provider.

use a3s_code_core::config::{CodeConfig, StorageBackend};
use serde_json::{json, Value};
use std::path::PathBuf;

fn summary(config: &CodeConfig) -> Value {
    json!({
        "default_model": config.default_model,
        "models": config.list_models().into_iter()
            .map(|(provider, model)| format!("{}/{}", provider.name, model.id))
            .collect::<Vec<_>>(),
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
    let path = std::env::var_os("A3S_CONFIG_FILE")
        .map(PathBuf::from)
        .expect("A3S_CONFIG_FILE must explicitly select the trusted ACL");
    let config = CodeConfig::from_file(&path).unwrap_or_else(|_| {
        panic!("selected ACL could not be parsed; source/credentials withheld")
    });
    println!("LONG_HORIZON_PREFLIGHT {}", summary(&config));
}

#[tokio::test]
#[ignore = "one bounded real-provider health probe; not ACP E2E acceptance"]
async fn selected_real_provider_health_without_account_or_session_activation() {
    use a3s_code_core::llm::{create_client_with_config, Message, NonRetryableLlmError};
    use std::time::{Duration, Instant};

    let path = std::env::var_os("A3S_CONFIG_FILE")
        .map(PathBuf::from)
        .expect("A3S_CONFIG_FILE must explicitly select the trusted ACL");
    let selected = std::env::var("A3S_LIVE_MODEL")
        .expect("A3S_LIVE_MODEL must select one exact declared provider/model");
    let (provider, model) = selected
        .split_once('/')
        .filter(|(provider, model)| !provider.is_empty() && !model.is_empty())
        .expect("A3S_LIVE_MODEL must be provider/model");
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
