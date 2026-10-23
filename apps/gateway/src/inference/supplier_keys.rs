//! Supplier secrets the console stores for the data plane.
//!
//! a3s-gateway asks a3s-api for enabled supplier keys and matches each keyed
//! inference target to the supplier with the same service name. The process
//! environment is not a second source for those keys.

use std::collections::HashMap;
use std::time::Duration;

use http::HeaderValue;
use uuid::Uuid;

use crate::config::{GatewayConfig, InferenceConfig};

/// One opened supplier secret. The name matches an inference target `service`.
pub(crate) struct SupplierCredential {
    pub(crate) name: String,
    pub(crate) api_key: String,
}

const UPSTREAM_KEYS_PATH: &str = "/v1/api/admin/internal/model-service/upstream-keys";
const FETCH_ATTEMPTS: u32 = 3;

/// Load console supplier keys before an inference snapshot is built.
///
/// An unreachable console or a refused credential leaves the list empty, which
/// disables keyed targets. There is no environment-variable fallback.
pub(crate) async fn load_supplier_credentials(config: &GatewayConfig) -> Vec<SupplierCredential> {
    let Some(origin) = console_api_origin(config) else {
        tracing::warn!("Inference supplier keys skipped: a3s-api service has no http server");
        return Vec::new();
    };
    let Some(token) = std::env::var("A3S_GATEWAY_ADMIN_TOKEN")
        .ok()
        .map(|value| value.trim().to_owned())
        .filter(|value| !value.is_empty())
    else {
        tracing::warn!("Inference supplier keys skipped: A3S_GATEWAY_ADMIN_TOKEN is unset");
        return Vec::new();
    };
    for attempt in 1..=FETCH_ATTEMPTS {
        match fetch_supplier_credentials(&origin, &token).await {
            Ok(keys) => {
                tracing::info!(suppliers = keys.len(), "Loaded console supplier keys");
                return keys;
            }
            Err(error) => {
                tracing::warn!(attempt, error = %error, "Console supplier keys unavailable");
                if attempt < FETCH_ATTEMPTS {
                    tokio::time::sleep(Duration::from_secs(1)).await;
                }
            }
        }
    }
    Vec::new()
}

pub(crate) fn console_api_origin(config: &GatewayConfig) -> Option<String> {
    let service = config.services.get("a3s-api")?;
    let url = service
        .load_balancer
        .servers
        .first()?
        .url
        .trim()
        .trim_end_matches('/');
    (url.starts_with("http://") || url.starts_with("https://")).then(|| url.to_owned())
}

async fn fetch_supplier_credentials(
    origin: &str,
    token: &str,
) -> Result<Vec<SupplierCredential>, String> {
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(5))
        .build()
        .map_err(|error| format!("supplier key client: {error}"))?;
    let response = client
        .get(format!("{origin}{UPSTREAM_KEYS_PATH}"))
        .bearer_auth(token)
        .send()
        .await
        .map_err(|error| format!("supplier key request failed: {error}"))?;
    if !response.status().is_success() {
        return Err(format!(
            "supplier key request returned HTTP {}",
            response.status()
        ));
    }
    let body = response
        .bytes()
        .await
        .map_err(|error| format!("supplier key body: {error}"))?;
    parse_supplier_credentials(&body)
}

pub(crate) fn parse_supplier_credentials(body: &[u8]) -> Result<Vec<SupplierCredential>, String> {
    let value: serde_json::Value = serde_json::from_slice(body)
        .map_err(|_| "supplier key response was not json".to_owned())?;
    if value.get("success") == Some(&serde_json::Value::Bool(false)) {
        return Err("supplier key response was refused".to_owned());
    }
    let items = value
        .get("data")
        .and_then(serde_json::Value::as_array)
        .ok_or_else(|| "supplier key response has no data array".to_owned())?;
    let mut keys = Vec::new();
    for item in items {
        let name = item
            .get("name")
            .and_then(serde_json::Value::as_str)
            .map(str::trim)
            .filter(|name| !name.is_empty())
            .ok_or_else(|| "supplier key is missing a name".to_owned())?;
        let api_key = item
            .get("apiKey")
            .and_then(serde_json::Value::as_str)
            .map(str::trim)
            .filter(|key| !key.is_empty())
            .ok_or_else(|| format!("supplier {name} key is empty"))?;
        keys.push(SupplierCredential {
            name: name.to_owned(),
            api_key: api_key.to_owned(),
        });
    }
    Ok(keys)
}

/// Keyed targets take the console secret whose name matches `service`.
/// A target with `api_key_env` and no matching secret is left out.
pub(crate) fn resolve_supplier_keys(
    policy: &InferenceConfig,
    suppliers: &[SupplierCredential],
) -> HashMap<Uuid, HeaderValue> {
    let mut keys = HashMap::new();
    for route in policy.routes.values() {
        for model in route.models.values() {
            for target in &model.targets {
                remember_console_key(&mut keys, suppliers, target);
            }
        }
        if let Some(target) = &route.ocr_supplier {
            remember_console_key(&mut keys, suppliers, target);
        }
        if let Some(target) = &route.decision_supplier {
            remember_console_key(&mut keys, suppliers, target);
        }
    }
    keys
}

fn remember_console_key(
    keys: &mut HashMap<Uuid, HeaderValue>,
    suppliers: &[SupplierCredential],
    target: &crate::config::InferenceTargetConfig,
) {
    if target.api_key_env.is_none() {
        return;
    }
    let Some(secret) = suppliers.iter().find_map(|supplier| {
        supplier
            .name
            .eq_ignore_ascii_case(&target.service)
            .then_some(supplier.api_key.trim())
            .filter(|secret| !secret.is_empty())
    }) else {
        tracing::warn!(
            target_id = %target.target_id,
            service = %target.service,
            "Inference target disabled: console supplier key is missing"
        );
        return;
    };
    match HeaderValue::from_str(&target.protocol.authorization_value(secret)) {
        Ok(mut value) => {
            value.set_sensitive(true);
            keys.insert(target.target_id, value);
        }
        Err(_) => tracing::warn!(
            target_id = %target.target_id,
            service = %target.service,
            "Inference target disabled: console supplier key is not a valid header"
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{
        InferenceModelConfig, InferenceProtocol, InferenceRouteConfig, InferenceTargetConfig,
        INFERENCE_TOKENIZER_REVISION,
    };

    #[test]
    fn parses_the_console_envelope_and_drops_nothing_named() {
        let body = br#"{"success":true,"data":[{"name":"Boyue","apiKey":"sk-console"}]}"#;
        let keys = parse_supplier_credentials(body).unwrap();
        assert_eq!(keys.len(), 1);
        assert_eq!(keys[0].name, "Boyue");
        assert_eq!(keys[0].api_key, "sk-console");
        assert!(parse_supplier_credentials(br#"{"success":false,"data":[]}"#).is_err());
    }

    #[test]
    fn matches_the_supplier_name_to_the_service_and_ignores_a_missing_one() {
        let boyue = Uuid::from_u128(1);
        let other = Uuid::from_u128(2);
        let route_id = Uuid::from_u128(3);
        let policy = InferenceConfig {
            expires_at: chrono::Utc::now() + chrono::Duration::hours(1),
            tokenizer_revision: INFERENCE_TOKENIZER_REVISION.to_owned(),
            credentials: HashMap::new(),
            workers: HashMap::new(),
            guardrails: HashMap::new(),
            routes: HashMap::from([(
                route_id,
                InferenceRouteConfig {
                    engine_profiles: Vec::new(),
                    route_id,
                    router: "openai".into(),
                    environment_id: Uuid::nil(),
                    policy_revision: 1,
                    grants: HashMap::new(),
                    listings: HashMap::new(),
                    console_catalog: None,
                    wire: Default::default(),
                    principal: None,
                    ocr_supplier: None,
                    decision_supplier: None,
                    embedding_supplier: None,
                    rerank_supplier: None,
                    models: HashMap::from([(
                        "deepseek-v4-flash".into(),
                        InferenceModelConfig {
                            engine_profile: None,
                            routing: None,
                            failover: Default::default(),
                            pricing: None,
                            latency_bias: false,
                            prefix_affinity: false,
                            prefix_parallelism: 1,
                            model_id: Uuid::from_u128(4),
                            scheduling: None,
                            targets: vec![
                                InferenceTargetConfig {
                                    target_id: boyue,
                                    service: "boyue".into(),
                                    upstream_model: "deepseek-v4-flash".into(),
                                    priority: 0,
                                    weight: 1,
                                    protocol: InferenceProtocol::Openai,
                                    api_key_env: Some("A3S_BOYUE_API_KEY".into()),
                                    translate: false,
                                },
                                InferenceTargetConfig {
                                    target_id: other,
                                    service: "other".into(),
                                    upstream_model: "m".into(),
                                    priority: 0,
                                    weight: 1,
                                    protocol: InferenceProtocol::Anthropic,
                                    api_key_env: Some("A3S_OTHER_API_KEY".into()),
                                    translate: false,
                                },
                            ],
                        },
                    )]),
                },
            )]),
        };
        let resolved = resolve_supplier_keys(
            &policy,
            &[SupplierCredential {
                name: "Boyue".into(),
                api_key: "sk-console".into(),
            }],
        );
        assert_eq!(
            resolved.get(&boyue).and_then(|value| value.to_str().ok()),
            Some("Bearer sk-console")
        );
        assert!(!resolved.contains_key(&other));
    }
}
