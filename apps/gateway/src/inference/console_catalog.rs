//! Console-managed model catalog delivery (spec 002).
//!
//! Routes that declare a `console_catalog` block serve whatever the console
//! (a3s-api) publishes: the poller fetches the compiled catalog, merges it
//! into a config clone (models, synthesized supplier services, failover
//! policies), and applies it through the standard reload path under the
//! lifecycle lock. A failed fetch keeps the last applied snapshot — the
//! serving plane never degrades to an empty catalog. The first successful
//! poll at boot replaces the route's initially empty model set; until then
//! catalog aliases answer with the named not-granted refusal.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use serde::Deserialize;

use crate::config::{
    ConsoleCatalogConfig, FailoverPolicy, GatewayConfig, InferenceModelConfig, InferenceProtocol,
    InferenceTargetConfig,
};

const CATALOG_PATH: &str = "/v1/api/admin/internal/model-service/catalog";
const FETCH_ATTEMPTS: u32 = 3;

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CatalogRoute {
    alias: String,
    failover: String,
    targets: Vec<CatalogTarget>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CatalogTarget {
    supplier_id: String,
    name: String,
    protocol: String,
    base_url: String,
    upstream_model: String,
    priority: u32,
    weight: u32,
}

#[derive(Debug, Deserialize)]
struct CatalogEnvelope {
    success: bool,
    data: Option<CatalogBody>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CatalogBody {
    routes: Vec<CatalogRoute>,
}

/// Spawns the poller when any inference route declares a console catalog.
pub(crate) fn maybe_spawn_catalog_poller(gateway: Arc<crate::gateway::Gateway>) {
    // Snapshot the source config at spawn: the applied config drops the
    // catalog marker, so the poller must remember which routes it owns.
    let bootstrap = gateway.current_effective_config();
    let configs = catalog_configs(&bootstrap);
    if configs.is_empty() {
        return;
    }
    let origin_service = configs[0].api_service.clone();
    let interval = poll_interval(&bootstrap);
    let managed_routes = bootstrap
        .inference
        .as_ref()
        .map(|inference| {
            inference
                .routes
                .values()
                .filter(|route| route.console_catalog.is_some())
                .filter_map(|route| {
                    route
                        .console_catalog
                        .clone()
                        .map(|catalog| (route.route_id, catalog))
                })
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    tokio::spawn(async move {
        poll_loop(gateway, origin_service, interval, managed_routes).await;
    });
}

fn catalog_configs(config: &GatewayConfig) -> Vec<ConsoleCatalogConfig> {
    config
        .inference
        .as_ref()
        .map(|inference| {
            inference
                .routes
                .values()
                .filter_map(|route| route.console_catalog.clone())
                .collect::<Vec<_>>()
        })
        .unwrap_or_default()
}

fn poll_interval(config: &GatewayConfig) -> Duration {
    let seconds = catalog_configs(config)
        .iter()
        .map(|catalog| catalog.poll_interval_secs)
        .min()
        .unwrap_or(30)
        .max(1);
    Duration::from_secs(seconds)
}

async fn poll_loop(
    gateway: Arc<crate::gateway::Gateway>,
    api_service: String,
    interval: Duration,
    mut managed_routes: Vec<(uuid::Uuid, ConsoleCatalogConfig)>,
) {
    let mut last_digest = String::new();
    loop {
        let config = gateway.current_effective_config();
        refresh_catalog_policies(&config, &mut managed_routes);
        match fetch_catalog(&config, &api_service).await {
            Ok(body) => {
                let digest = policy_digest(&body, &managed_routes);
                if digest == last_digest {
                    tracing::debug!("console catalog unchanged; keeping snapshot");
                } else {
                    let routes = match parse_catalog(&body) {
                        Ok(routes) => routes,
                        Err(error) => {
                            tracing::warn!(
                                error = %error,
                                "console catalog payload rejected; keeping last snapshot"
                            );
                            tokio::time::sleep(interval).await;
                            continue;
                        }
                    };
                    let mut managed = managed_routes.clone();
                    let applied = gateway
                        .reload_console_catalog(|current| {
                            refresh_catalog_policies(current, &mut managed);
                            merge_catalog_with_policy(current, &routes, &managed)
                        })
                        .await;
                    match applied {
                        Ok(()) => {
                            tracing::info!(routes = routes.len(), "console catalog applied");
                            managed_routes = managed;
                            last_digest = policy_digest(&body, &managed_routes);
                        }
                        Err(error) => {
                            tracing::warn!(
                                error = %error,
                                "console catalog apply refused; keeping last snapshot"
                            );
                            // Do not record the digest: retry on the next tick.
                        }
                    }
                }
            }
            Err(error) => {
                tracing::warn!(
                    error = %error,
                    "console catalog fetch failed; keeping last snapshot"
                );
            }
        }
        tokio::time::sleep(interval).await;
    }
}

fn refresh_catalog_policies(
    config: &GatewayConfig,
    policies: &mut [(uuid::Uuid, ConsoleCatalogConfig)],
) {
    let Some(inference) = &config.inference else {
        return;
    };
    for (id, policy) in policies {
        if let Some(source) = inference
            .routes
            .get(id)
            .and_then(|route| route.console_catalog.as_ref())
        {
            *policy = source.clone();
        }
    }
}

fn policy_digest(body: &str, policies: &[(uuid::Uuid, ConsoleCatalogConfig)]) -> String {
    // String-only policy serialization cannot fail. Hash each part separately
    // so arbitrary catalog bytes cannot collide with a policy boundary.
    let mut material = digest_of(body);
    for (id, policy) in policies {
        material.push_str(&id.to_string());
        material.push_str(&digest_of(&policy.api_service));
        material.push_str(&format!(
            ":{}:{}:{}:",
            policy.poll_interval_secs, policy.prefix_affinity, policy.prefix_parallelism
        ));
        for supplier in &policy.excluded_supplier_ids {
            material.push_str(&digest_of(supplier));
        }
        material.push(';');
    }
    digest_of(&material)
}

fn parse_catalog(body: &str) -> Result<Vec<CatalogRoute>, String> {
    let envelope: CatalogEnvelope =
        serde_json::from_str(body).map_err(|error| format!("catalog json: {error}"))?;
    if !envelope.success {
        return Err("catalog envelope reported failure".to_string());
    }
    let parsed: CatalogBody = envelope.data.ok_or("catalog envelope carried no data")?;
    for route in &parsed.routes {
        FailoverPolicy::parse(&route.failover)
            .map_err(|error| format!("route {}: {error}", route.alias))?;
        for target in &route.targets {
            use std::str::FromStr;
            InferenceProtocol::from_str(&target.protocol)
                .map_err(|error| format!("route {} target: {error}", route.alias))?;
        }
    }
    Ok(parsed.routes)
}

/// Pure merge: catalog routes replace the `models` of every console-catalog
/// route; each distinct supplier becomes a synthesized service (no health
/// check — a probe failure must not take model routing offline) and the
/// model targets reference it by supplier name, matching how upstream keys
/// are keyed.
#[cfg(test)]
fn merge_catalog(
    current: &GatewayConfig,
    routes: &[CatalogRoute],
    managed_routes: &[uuid::Uuid],
) -> GatewayConfig {
    let policies = managed_routes
        .iter()
        .filter_map(|id| {
            let route = current
                .inference
                .as_ref()?
                .routes
                .values()
                .find(|route| route.route_id == *id)?;
            let catalog = route
                .console_catalog
                .clone()
                .unwrap_or(ConsoleCatalogConfig {
                    prefix_parallelism: route
                        .models
                        .values()
                        .next()
                        .map_or(1, |model| model.prefix_parallelism),
                    api_service: String::new(),
                    poll_interval_secs: 30,
                    prefix_affinity: !route.models.is_empty()
                        && route.models.values().all(|model| model.prefix_affinity),
                    excluded_supplier_ids: Vec::new(),
                });
            Some((*id, catalog))
        })
        .collect::<Vec<_>>();
    merge_catalog_with_policy(current, routes, &policies)
}

fn merge_catalog_with_policy(
    current: &GatewayConfig,
    routes: &[CatalogRoute],
    policies: &[(uuid::Uuid, ConsoleCatalogConfig)],
) -> GatewayConfig {
    let mut merged = current.clone();
    let Some(inference) = merged.inference.as_mut() else {
        return merged;
    };
    for route_config in inference.routes.values_mut() {
        // The applied snapshot drops the catalog marker; ownership is by
        // the route IDs captured at spawn.
        let Some((_, policy)) = policies.iter().find(|(id, _)| *id == route_config.route_id) else {
            continue;
        };
        // The source marker owns the opt-in. Applied snapshots drop that
        // marker, so subsequent polls inherit only a uniform concrete policy.
        let prefix_affinity = policy.prefix_affinity;
        let mut models = HashMap::new();
        for catalog_route in routes {
            let failover =
                FailoverPolicy::parse(&catalog_route.failover).unwrap_or(FailoverPolicy::None);
            let mut targets = Vec::with_capacity(catalog_route.targets.len());
            for target in &catalog_route.targets {
                if policy.excluded_supplier_ids.contains(&target.supplier_id) {
                    continue;
                }
                targets.push(InferenceTargetConfig {
                    // Exact catalog descriptors, not poll timing, identify the
                    // target. Route namespace isolates independently granted pools.
                    target_id: catalog_target_id(
                        route_config.route_id,
                        &catalog_route.alias,
                        target,
                    ),
                    service: target.name.clone(),
                    upstream_model: target.upstream_model.clone(),
                    priority: target.priority,
                    weight: target.weight,
                    protocol: std::str::FromStr::from_str(&target.protocol)
                        .unwrap_or(InferenceProtocol::Openai),
                    // The marker the key resolver requires; the secret itself
                    // always comes from the console upstream-keys pull.
                    api_key_env: Some(format!(
                        "CONSOLE_SUPPLIER_{}",
                        target.supplier_id.replace('-', "_")
                    )),
                    translate: false,
                });
            }
            if targets.is_empty() {
                continue;
            }
            models.insert(
                catalog_route.alias.clone(),
                InferenceModelConfig {
                    prefix_parallelism: policy.prefix_parallelism,
                    model_id: uuid::Uuid::new_v5(
                        &route_config.route_id,
                        catalog_route.alias.as_bytes(),
                    ),
                    failover,
                    pricing: None,
                    latency_bias: false,
                    prefix_affinity,
                    targets,
                    scheduling: None,
                    routing: None,
                    engine_profile: None,
                },
            );
        }
        route_config.models = models;
        // The applied snapshot carries concrete models; the catalog marker
        // stays only on the source-of-truth config the poller reads.
        route_config.console_catalog = None;
    }
    // Synthesize one service per distinct supplier name and base URL.
    let mut suppliers: HashMap<String, String> = HashMap::new();
    for catalog_route in routes {
        for target in &catalog_route.targets {
            suppliers
                .entry(target.name.clone())
                .or_insert_with(|| origin_of(&target.base_url));
        }
    }
    if !suppliers.is_empty() {
        let services = &mut merged.services;
        for (name, url) in suppliers {
            services
                .entry(name)
                .or_insert_with(|| default_supplier_service(url));
        }
    }
    merged
}

fn catalog_target_id(route_id: uuid::Uuid, alias: &str, target: &CatalogTarget) -> uuid::Uuid {
    // Length framing preserves exact descriptor boundaries without guessing
    // aliases, supplier names, or URL meanings. Priority/weight are routing
    // policy, not execution identity, so changing them preserves the target ID.
    let mut identity = Vec::new();
    for part in [
        alias,
        &target.supplier_id,
        &target.name,
        &target.protocol,
        &target.base_url,
        &target.upstream_model,
    ] {
        identity.extend_from_slice(&(part.len() as u64).to_le_bytes());
        identity.extend_from_slice(part.as_bytes());
    }
    uuid::Uuid::new_v5(&route_id, &identity)
}

/// Reduces a console baseUrl to its origin: the native inference path
/// keeps the caller path (`/v1/chat/completions`), so a version suffix in
/// the stored baseUrl would double up (the ACL boyue precedent — "Origin
/// has no /v1 suffix").
fn origin_of(baseUrl: &str) -> String {
    // split 'https://host/v1' -> scheme 'https', authority 'host'
    let mut parts = baseUrl.splitn(4, '/');
    match (parts.next(), parts.next(), parts.next(), parts.next()) {
        (Some(scheme), Some(""), Some(authority), _) => {
            let scheme = scheme.trim_end_matches(':');
            format!("{scheme}://{authority}")
        }
        _ => baseUrl.to_string(),
    }
}

/// A supplier service with no health check: model routing must not go
/// offline on probe failures (the boyue precedent in the local spine ACL).
fn default_supplier_service(url: String) -> crate::config::ServiceConfig {
    crate::config::ServiceConfig {
        load_balancer: crate::config::LoadBalancerConfig {
            strategy: Default::default(),
            connect_timeout: "30s".to_string(),
            request_timeout: "120s".to_string(),
            stream_idle_timeout: "5m".to_string(),
            stream_total_timeout: crate::config::default_stream_total_timeout(),
            servers: vec![crate::config::ServerConfig {
                url,
                weight: 1,
                target: None,
            }],
            health_check: None,
            sticky: None,
            tls_ca_file: None,
        },
        scaling: None,
        revisions: Vec::new(),
        rollout: None,
        mirror: None,
        failover: None,
    }
}

fn digest_of(body: &str) -> String {
    use sha2::Digest;
    format!("{:x}", sha2::Sha256::digest(body.as_bytes()))
}

async fn fetch_catalog(config: &GatewayConfig, api_service: &str) -> Result<String, String> {
    let origin = config
        .services
        .get(api_service)
        .and_then(|service| service.load_balancer.servers.first())
        .map(|server| server.url.trim_end_matches('/').to_owned())
        .ok_or_else(|| format!("catalog service '{api_service}' has no server"))?;
    let token = std::env::var("A3S_GATEWAY_ADMIN_TOKEN")
        .ok()
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| "A3S_GATEWAY_ADMIN_TOKEN is unset".to_string())?;
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(10))
        .build()
        .map_err(|error| format!("catalog client: {error}"))?;
    let mut last_error = String::new();
    for attempt in 1..=FETCH_ATTEMPTS {
        let response = client
            .get(format!("{origin}{CATALOG_PATH}"))
            .bearer_auth(&token)
            .send()
            .await;
        match response {
            Ok(response) if response.status().is_success() => {
                let body = response
                    .text()
                    .await
                    .map_err(|error| format!("catalog body: {error}"))?;
                // Never treat an HTML error page as a catalog.
                if body.trim_start().starts_with('<') {
                    return Err("catalog response was not json".to_string());
                }
                return Ok(body);
            }
            Ok(response) => {
                last_error = format!("HTTP {}", response.status());
            }
            Err(error) => {
                last_error = error.to_string();
            }
        }
        if attempt < FETCH_ATTEMPTS {
            tokio::time::sleep(Duration::from_secs(attempt as u64)).await;
        }
    }
    Err(last_error)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn catalog_json(routes: &str) -> String {
        format!("{{\"success\":true,\"data\":{{\"routes\":{routes}}}}}")
    }

    #[test]
    fn prefix_affinity_catalog_descriptors_keep_target_identity() {
        let route_id = uuid::Uuid::new_v4();
        let mut target = CatalogTarget {
            supplier_id: "supplier-1".into(),
            name: "pool-1".into(),
            protocol: "openai".into(),
            base_url: "http://127.0.0.1:18200/v1".into(),
            upstream_model: "prism-27b".into(),
            priority: 0,
            weight: 1,
        };
        let first = catalog_target_id(route_id, "external-model", &target);
        assert_eq!(
            first,
            catalog_target_id(route_id, "external-model", &target)
        );
        target.weight = 3;
        target.priority = 2;
        assert_eq!(
            first,
            catalog_target_id(route_id, "external-model", &target)
        );
        assert_ne!(
            first,
            catalog_target_id(uuid::Uuid::new_v4(), "external-model", &target)
        );
        assert_ne!(first, catalog_target_id(route_id, "another-model", &target));
        target.upstream_model = "different-deployment".into();
        assert_ne!(
            first,
            catalog_target_id(route_id, "external-model", &target)
        );
    }

    #[test]
    fn prefix_affinity_catalog_reload_keeps_identity_and_honors_disabling() {
        let route_id = uuid::Uuid::new_v4();
        let mut current = GatewayConfig::default();
        current.inference = Some(serde_json::from_value(serde_json::json!({
            "expires_at":"2099-01-01T00:00:00Z", "tokenizer_revision":crate::config::INFERENCE_TOKENIZER_REVISION,
            "routes":{route_id.to_string():{
                "route_id":route_id,"environment_id":uuid::Uuid::new_v4(),"router":"models","policy_revision":1,
                "models":{},"grants":{},"console_catalog":{"api_service":"console","prefix_affinity":true}
            }}
        })).unwrap());
        let routes = parse_catalog(&catalog_json(r#"[{"alias":"m","failover":"none","targets":[
            {"supplierId":"supplier-1","name":"pool-1","protocol":"openai","baseUrl":"http://127.0.0.1:18200",
             "upstreamModel":"prism-27b","priority":0,"weight":1}]}]"#)).unwrap();
        let first = merge_catalog(&current, &routes, &[route_id]);
        let second = merge_catalog(&first, &routes, &[route_id]);
        let model = |config: &GatewayConfig| {
            config.inference.as_ref().unwrap().routes[&route_id].models["m"].clone()
        };
        assert_eq!(model(&first), model(&second));
        assert!(model(&second).prefix_affinity);
        let mut draining_routes = routes.clone();
        let mut sibling = draining_routes[0].targets[0].clone();
        sibling.supplier_id = "supplier-2".into();
        sibling.name = "pool-2".into();
        draining_routes[0].targets.push(sibling);
        let mut policy = current.inference.as_ref().unwrap().routes[&route_id]
            .console_catalog
            .clone()
            .unwrap();
        policy.excluded_supplier_ids.push("supplier-1".into());
        let policies = [(route_id, policy)];
        let drained = merge_catalog_with_policy(&current, &draining_routes, &policies);
        let repolled = merge_catalog_with_policy(&drained, &draining_routes, &policies);
        assert_eq!(model(&repolled).targets.len(), 1);
        assert_eq!(model(&repolled).targets[0].service, "pool-2");
        let restored = merge_catalog_with_policy(
            &repolled,
            &draining_routes,
            &[(
                route_id,
                current.inference.as_ref().unwrap().routes[&route_id]
                    .console_catalog
                    .clone()
                    .unwrap(),
            )],
        );
        assert_eq!(model(&restored).targets.len(), 2);
        let mut remembered = policies.to_vec();
        let before_policy_change = policy_digest("unchanged-catalog", &remembered);
        refresh_catalog_policies(&current, &mut remembered);
        assert_ne!(
            before_policy_change,
            policy_digest("unchanged-catalog", &remembered)
        );
        assert!(remembered[0].1.excluded_supplier_ids.is_empty());
        let mut changed_source = current.clone();
        changed_source
            .inference
            .as_mut()
            .unwrap()
            .routes
            .get_mut(&route_id)
            .unwrap()
            .console_catalog
            .as_mut()
            .unwrap()
            .prefix_affinity = false;
        let old_policy = policy_digest("unchanged-catalog", &remembered);
        refresh_catalog_policies(&changed_source, &mut remembered);
        assert_ne!(old_policy, policy_digest("unchanged-catalog", &remembered));
        let projected = merge_catalog_with_policy(&changed_source, &routes, &remembered);
        refresh_catalog_policies(&projected, &mut remembered);
        assert!(!remembered[0].1.prefix_affinity);
        assert!(
            !model(&merge_catalog_with_policy(&projected, &routes, &remembered)).prefix_affinity
        );
        // A source reload explicitly disabling locality must take precedence
        // over previously applied, concrete models on every later poll.
        let mut disabled = current;
        disabled
            .inference
            .as_mut()
            .unwrap()
            .routes
            .get_mut(&route_id)
            .unwrap()
            .console_catalog
            .as_mut()
            .unwrap()
            .prefix_affinity = false;
        let disabled = merge_catalog(&disabled, &routes, &[route_id]);
        let again = merge_catalog(&disabled, &routes, &[route_id]);
        assert!(!model(&again).prefix_affinity);
    }

    #[test]
    fn origin_strips_version_paths() {
        assert_eq!(
            origin_of("https://api.deepseek.com/v1"),
            "https://api.deepseek.com"
        );
        assert_eq!(
            origin_of("http://35.220.164.252:3888/v1"),
            "http://35.220.164.252:3888"
        );
        assert_eq!(origin_of("http://host:9000"), "http://host:9000");
    }

    #[test]
    fn parse_accepts_the_contract_shape() {
        let routes = r#"[{"alias":"deepseek-v3","failover":"none","targets":[
            {"supplierId":"11111111-1111-4111-8111-111111111111","name":"boyue",
             "protocol":"openai","baseUrl":"https://api.example.com",
             "upstreamModel":"ds-chat","priority":0,"weight":1}]}]"#;
        let parsed = parse_catalog(&catalog_json(routes)).unwrap();
        assert_eq!(parsed.len(), 1);
        assert_eq!(parsed[0].alias, "deepseek-v3");
        assert_eq!(parsed[0].targets[0].name, "boyue");
    }

    #[test]
    fn parse_refuses_unknown_failover_and_protocol() {
        let bad_failover = r#"[{"alias":"m","failover":"always","targets":[]}]"#;
        let error = parse_catalog(&catalog_json(bad_failover)).unwrap_err();
        assert!(error.contains("none") && error.contains("retryable_status"));

        let bad_protocol = r#"[{"alias":"m","failover":"none","targets":[
            {"supplierId":"x","name":"s","protocol":"grpc","baseUrl":"https://e",
             "upstreamModel":"m","priority":0,"weight":1}]}]"#;
        assert!(parse_catalog(&catalog_json(bad_protocol)).is_err());
    }
}
