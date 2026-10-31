//! Cloud hosted Function service traffic compiled into ordinary Gateway routes.
//!
//! The control plane declares one immutable intent per path. This module is the
//! data-plane consumer: a healthy exact revision is proxied, and a missing
//! target or a drifted generation fails closed without dialing that upstream.

use super::service::{
    default_connect_timeout, default_request_timeout, default_stream_idle_timeout,
    default_stream_total_timeout, validate_server_url,
};
use super::{
    GatewayConfig, LoadBalancerConfig, ManagedTargetConfig, MiddlewareConfig, RouterConfig,
    ServerConfig, ServiceConfig, Strategy,
};
use crate::error::{GatewayError, Result};
use crate::hosted_function_activation::{HostedFunctionActivation, DEFAULT_RETRY_AFTER_SECS};
use aster_acl::{Block, Value};
use std::collections::BTreeMap;

pub(crate) const FUNCTION_ENTRYPOINT: &str = "aster-cloud-function";
const FAIL_CLOSED_UPSTREAM: &str = "http://127.0.0.1:9/";

struct HostedFunctionIntent {
    path: String,
    revision_id: uuid::Uuid,
    revision_generation: u64,
    /// Function profile concurrency ceiling. Gateway admits this many in-flight
    /// requests and does not start a scaler.
    max_concurrency: Option<u32>,
    target: Option<HostedFunctionTarget>,
    activation: Option<HostedFunctionActivation>,
}

struct HostedFunctionTarget {
    unit_id: String,
    generation: u64,
    upstream: String,
    /// Revision the target actually serves. When present it alone decides drift,
    /// so a new incarnation of the same revision (activation, restart,
    /// evacuation) keeps serving; absent on legacy snapshots, which keep the
    /// generation pin.
    revision_id: Option<uuid::Uuid>,
}

impl HostedFunctionTarget {
    fn serves(&self, revision_id: uuid::Uuid, revision_generation: u64) -> bool {
        match self.revision_id {
            Some(target_revision) => target_revision == revision_id,
            None => self.generation == revision_generation,
        }
    }
}

pub(super) fn install_hosted_function_routes(
    config: &mut GatewayConfig,
    block: &Block,
) -> Result<()> {
    let intents = parse_hosted_function_intents(block)?;
    materialize_hosted_function_routes(config, &intents)
}

fn parse_hosted_function_intents(block: &Block) -> Result<Vec<HostedFunctionIntent>> {
    if !block.attributes.is_empty() {
        return Err(config_error(
            "hosted_function_service_edge_traffic_intents accepts only intent blocks",
        ));
    }
    let mut intents = Vec::new();
    let mut paths = BTreeMap::<String, ()>::new();
    for child in &block.blocks {
        if child.name != "intents" {
            return Err(config_error(format!(
                "hosted Function traffic block '{}' is not an intent",
                child.name
            )));
        }
        let intent = parse_intent(child)?;
        if paths.insert(intent.path.clone(), ()).is_some() {
            return Err(config_error(format!(
                "hosted Function traffic path '{}' is duplicated",
                intent.path
            )));
        }
        intents.push(intent);
    }
    Ok(intents)
}

fn materialize_hosted_function_routes(
    config: &mut GatewayConfig,
    intents: &[HostedFunctionIntent],
) -> Result<()> {
    if intents.is_empty() {
        return Ok(());
    }
    if !config.entrypoints.contains_key(FUNCTION_ENTRYPOINT) {
        return Err(config_error(
            "hosted Function traffic requires entrypoint aster-cloud-function",
        ));
    }
    let mut ordered = intents.iter().collect::<Vec<_>>();
    ordered.sort_by(|left, right| left.path.cmp(&right.path));
    for (index, intent) in ordered.into_iter().enumerate() {
        let name = hosted_function_service_name(index, intent.max_concurrency);
        if config.routers.contains_key(&name) || config.services.contains_key(&name) {
            return Err(config_error(format!(
                "hosted Function route '{name}' collides with an existing Gateway route"
            )));
        }
        let rule = format!("PathPrefix(`{}`)", intent.path);
        let healthy = match &intent.target {
            Some(target) if target.serves(intent.revision_id, intent.revision_generation) => {
                Some(target)
            }
            _ => None,
        };
        if let Some(target) = healthy {
            config.routers.insert(
                name.clone(),
                RouterConfig {
                    rule,
                    service: name.clone(),
                    entrypoints: vec![FUNCTION_ENTRYPOINT.to_owned()],
                    middlewares: Vec::new(),
                    // Unset priority so a longer function path outranks a shorter PathPrefix.
                    priority: 0,
                    parking: None,
                },
            );
            config
                .services
                .insert(name, service_for_upstream(target, intent.revision_id)?);
            continue;
        }
        let middleware_name = format!("{name}-unavailable");
        let code = if intent.target.is_some() {
            "hosted_function_revision_drift"
        } else {
            if let Some(activation) = &intent.activation {
                config
                    .hosted_function_activations
                    .insert(name.clone(), activation.clone());
            }
            "hosted_function_unavailable"
        };
        config.middlewares.insert(
            middleware_name.clone(),
            MiddlewareConfig {
                middleware_type: "fallback".to_owned(),
                status_code: Some(503),
                value: Some(format!(r#"{{"code":"{code}"}}"#)),
                header: Some("application/json".to_owned()),
                ..MiddlewareConfig::default()
            },
        );
        config.routers.insert(
            name.clone(),
            RouterConfig {
                rule,
                service: name.clone(),
                entrypoints: vec![FUNCTION_ENTRYPOINT.to_owned()],
                middlewares: vec![middleware_name],
                // Unset priority so a longer function path outranks a shorter PathPrefix.
                priority: 0,
                parking: None,
            },
        );
        config.services.insert(
            name,
            ServiceConfig {
                load_balancer: LoadBalancerConfig {
                    strategy: Strategy::RoundRobin,
                    connect_timeout: default_connect_timeout(),
                    request_timeout: default_request_timeout(),
                    stream_idle_timeout: default_stream_idle_timeout(),
                    stream_total_timeout: default_stream_total_timeout(),
                    servers: vec![ServerConfig {
                        url: FAIL_CLOSED_UPSTREAM.to_owned(),
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
            },
        );
    }
    Ok(())
}

fn service_for_upstream(
    target: &HostedFunctionTarget,
    revision_id: uuid::Uuid,
) -> Result<ServiceConfig> {
    validate_server_url(&target.upstream).map_err(|error| {
        config_error(format!(
            "hosted Function upstream '{}' is invalid: {error}",
            target.upstream
        ))
    })?;
    let parsed = url::Url::parse(&target.upstream).map_err(|error| {
        config_error(format!(
            "hosted Function upstream '{}' is invalid: {error}",
            target.upstream
        ))
    })?;
    if parsed.scheme() != "http" && parsed.scheme() != "https" {
        return Err(config_error(format!(
            "hosted Function upstream '{}' must be http or https",
            target.upstream
        )));
    }
    let managed = ManagedTargetConfig {
        target_id: revision_id,
        unit_id: target.unit_id.clone(),
        generation: target.generation,
    };
    managed.validate().map_err(|error| {
        config_error(format!(
            "hosted Function managed target is invalid: {error}"
        ))
    })?;
    Ok(ServiceConfig {
        load_balancer: LoadBalancerConfig {
            strategy: Strategy::RoundRobin,
            connect_timeout: default_connect_timeout(),
            request_timeout: default_request_timeout(),
            stream_idle_timeout: default_stream_idle_timeout(),
            stream_total_timeout: default_stream_total_timeout(),
            servers: vec![ServerConfig {
                url: target.upstream.clone(),
                weight: 1,
                target: Some(managed),
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
    })
}

fn parse_intent(block: &Block) -> Result<HostedFunctionIntent> {
    let traffic = required_child(block, "traffic")?;
    let path = required_string(traffic, "path")?;
    if !valid_path(&path) {
        return Err(config_error(format!(
            "hosted Function traffic path '{path}' is not a canonical absolute path"
        )));
    }
    if block.labels.first().is_some_and(|label| label != &path) {
        return Err(config_error(
            "hosted Function intent label and traffic path disagree",
        ));
    }
    let protocol = required_string(traffic, "protocol")?;
    if protocol != "http" && protocol != "mcp_sessionless" {
        return Err(config_error(format!(
            "hosted Function traffic protocol '{protocol}' is unsupported"
        )));
    }
    let workload = required_child(block, "workload")?;
    let revision_id = required_string(workload, "revision_id")?;
    let revision_id = uuid::Uuid::parse_str(&revision_id)
        .map_err(|_| config_error("hosted Function workload revision_id is not a UUID"))?;
    if revision_id.is_nil() {
        return Err(config_error(
            "hosted Function workload revision_id must not be nil",
        ));
    }
    let revision_generation = required_u64(workload, "revision_generation")?;
    if revision_generation == 0 {
        return Err(config_error(
            "hosted Function workload revision_generation must be positive",
        ));
    }
    let max_concurrency = optional_concurrency_ceiling(workload)?;
    let target = match child(block, "target") {
        None => None,
        Some(target) => Some(HostedFunctionTarget {
            unit_id: required_string(target, "unit_id")?,
            generation: required_u64(target, "generation")?,
            upstream: required_string(target, "upstream")?,
            revision_id: optional_uuid(target, "revision_id")?,
        }),
    };
    let activation = parse_activation(block, workload)?;
    Ok(HostedFunctionIntent {
        path,
        revision_id,
        revision_generation,
        max_concurrency,
        target,
        activation,
    })
}

/// Activation identity comes from the intent's Cloud scope. Intents without it
/// (standalone fixtures) record no demand; an explicit `activation` block adds a
/// bounded hold and Retry-After for scale-to-zero Workloads.
fn parse_activation(block: &Block, workload: &Block) -> Result<Option<HostedFunctionActivation>> {
    let organization_id = optional_uuid(block, "organization_id")?;
    let workload_id = optional_uuid(workload, "workload_id")?;
    let declared = child(block, "activation");
    let (Some(organization_id), Some(workload_id)) = (organization_id, workload_id) else {
        if declared.is_some() {
            return Err(config_error(
                "hosted Function activation requires organization_id and workload.workload_id",
            ));
        }
        return Ok(None);
    };
    let (hold, retry_after_secs) = match declared {
        None => (None, DEFAULT_RETRY_AFTER_SECS),
        Some(activation) => (
            optional_u64(activation, "hold_ms")?.map(std::time::Duration::from_millis),
            optional_u64(activation, "retry_after_s")?.unwrap_or(DEFAULT_RETRY_AFTER_SECS),
        ),
    };
    let activation = HostedFunctionActivation {
        organization_id,
        workload_id,
        hold,
        retry_after_secs,
    };
    activation.validate().map_err(config_error)?;
    Ok(Some(activation))
}

fn optional_uuid(block: &Block, key: &str) -> Result<Option<uuid::Uuid>> {
    match block.attributes.get(key) {
        None => Ok(None),
        Some(Value::String(value)) => uuid::Uuid::parse_str(value)
            .ok()
            .filter(|uuid| !uuid.is_nil())
            .map(Some)
            .ok_or_else(|| config_error(format!("hosted Function field '{key}' is not a UUID"))),
        Some(_) => Err(config_error(format!(
            "hosted Function field '{key}' must be a UUID string"
        ))),
    }
}

fn hosted_function_service_name(index: usize, ceiling: Option<u32>) -> String {
    match ceiling {
        Some(limit) => format!("hosted-function-{index}-ceiling-{limit}"),
        None => format!("hosted-function-{index}"),
    }
}

/// Admission ceiling encoded in a hosted Function service name.
///
/// The ceiling stays off `ScalingConfig`: a positive `container_concurrency`
/// starts the standalone autoscaler, and Workloads already owns scale-to-zero.
pub(crate) fn hosted_function_admission_ceiling(service_name: &str) -> Option<u32> {
    let rest = service_name.strip_prefix("hosted-function-")?;
    let (_index, ceiling) = rest.rsplit_once("-ceiling-")?;
    let limit = ceiling.parse::<u32>().ok()?;
    (limit > 0).then_some(limit)
}

fn valid_path(path: &str) -> bool {
    path.starts_with('/')
        && path.len() > 1
        && path.len() <= 512
        && !path.contains(['`', '"', '\'', ' ', '\n', '\r', '\0', '\\'])
}

fn child<'a>(block: &'a Block, name: &str) -> Option<&'a Block> {
    block.blocks.iter().find(|child| child.name == name)
}

fn required_child<'a>(block: &'a Block, name: &str) -> Result<&'a Block> {
    child(block, name)
        .ok_or_else(|| config_error(format!("hosted Function intent requires a {name} block")))
}

fn required_string(block: &Block, key: &str) -> Result<String> {
    match block.attributes.get(key) {
        Some(Value::String(value)) if !value.is_empty() => Ok(value.clone()),
        Some(_) => Err(config_error(format!(
            "hosted Function field '{key}' must be a non-empty string"
        ))),
        None => Err(config_error(format!(
            "hosted Function field '{key}' is required"
        ))),
    }
}

fn optional_concurrency_ceiling(block: &Block) -> Result<Option<u32>> {
    let Some(value) = optional_u64(block, "max_concurrency")? else {
        return Ok(None);
    };
    if value == 0 {
        return Err(config_error(
            "hosted Function max_concurrency must be positive",
        ));
    }
    u32::try_from(value).map(Some).map_err(|_| {
        config_error("hosted Function max_concurrency exceeds the gateway admission limit")
    })
}

fn optional_u64(block: &Block, key: &str) -> Result<Option<u64>> {
    match block.attributes.get(key) {
        None => Ok(None),
        Some(Value::Number(value))
            if value.is_finite() && *value >= 0.0 && value.fract() == 0.0 =>
        {
            Ok(Some(*value as u64))
        }
        Some(_) => Err(config_error(format!(
            "hosted Function field '{key}' must be a non-negative integer"
        ))),
    }
}

fn required_u64(block: &Block, key: &str) -> Result<u64> {
    match block.attributes.get(key) {
        Some(Value::Number(value))
            if value.is_finite() && *value >= 0.0 && value.fract() == 0.0 =>
        {
            Ok(*value as u64)
        }
        Some(_) => Err(config_error(format!(
            "hosted Function field '{key}' must be a non-negative integer"
        ))),
        None => Err(config_error(format!(
            "hosted Function field '{key}' is required"
        ))),
    }
}

fn config_error(message: impl Into<String>) -> GatewayError {
    GatewayError::Config(message.into())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entrypoint_acl(body: &str) -> String {
        format!(
            r#"
            entrypoints "{FUNCTION_ENTRYPOINT}" {{
              address = "127.0.0.1:18081"
            }}
            {body}
            "#
        )
    }

    fn intent(target: &str) -> String {
        format!(
            r#"
            hosted_function_service_edge_traffic_intents {{
              intents "/functions/sessionless-mcp" {{
                traffic {{
                  path = "/functions/sessionless-mcp"
                  protocol = "mcp_sessionless"
                  visibility = "public"
                  runtime_port = "http"
                }}
                workload {{
                  workload_id = "88888888-8888-4888-8888-888888888888"
                  revision_id = "99999999-9999-4999-8999-999999999999"
                  revision_generation = 4
                }}
                {target}
              }}
            }}
            "#
        )
    }

    #[test]
    fn healthy_exact_revision_becomes_one_upstream_route() {
        let config = GatewayConfig::from_acl(&entrypoint_acl(&intent(
            r#"
            target {
              unit_id = "workload:88888888-8888-4888-8888-888888888888:revision:99999999-9999-4999-8999-999999999999"
              generation = 4
              upstream = "http://127.0.0.1:49152/"
            }
            "#,
        )))
        .expect("parse");
        let router = config.routers.get("hosted-function-0").expect("router");
        assert_eq!(router.rule, "PathPrefix(`/functions/sessionless-mcp`)");
        assert!(router.middlewares.is_empty());
        assert_eq!(router.entrypoints, vec![FUNCTION_ENTRYPOINT]);
        let service = config.services.get("hosted-function-0").expect("service");
        assert_eq!(
            service.load_balancer.servers[0].url,
            "http://127.0.0.1:49152/"
        );
        let target = service.load_balancer.servers[0]
            .target
            .as_ref()
            .expect("managed target");
        assert_eq!(target.generation, 4);
        assert_eq!(
            target.target_id,
            uuid::Uuid::parse_str("99999999-9999-4999-8999-999999999999").unwrap()
        );
    }

    #[test]
    fn missing_target_fails_closed_without_an_upstream() {
        let config = GatewayConfig::from_acl(&entrypoint_acl(&intent(""))).expect("parse");
        let router = config.routers.get("hosted-function-0").expect("router");
        assert_eq!(router.middlewares, vec!["hosted-function-0-unavailable"]);
        let middleware = config
            .middlewares
            .get("hosted-function-0-unavailable")
            .expect("middleware");
        assert_eq!(middleware.middleware_type, "fallback");
        assert_eq!(middleware.status_code, Some(503));
        assert!(middleware
            .value
            .as_deref()
            .unwrap()
            .contains("hosted_function_unavailable"));
        assert_eq!(
            config.services["hosted-function-0"].load_balancer.servers[0].url,
            FAIL_CLOSED_UPSTREAM
        );
    }

    #[test]
    fn drifted_generation_fails_closed_without_using_the_upstream() {
        let config = GatewayConfig::from_acl(&entrypoint_acl(&intent(
            r#"
            target {
              unit_id = "workload:unit"
              generation = 3
              upstream = "http://127.0.0.1:49152/"
            }
            "#,
        )))
        .expect("parse");
        let middleware = &config.middlewares["hosted-function-0-unavailable"];
        assert!(middleware
            .value
            .as_deref()
            .unwrap()
            .contains("hosted_function_revision_drift"));
        assert_eq!(
            config.services["hosted-function-0"].load_balancer.servers[0].url,
            FAIL_CLOSED_UPSTREAM
        );
    }

    #[test]
    fn longer_function_path_is_not_captured_by_a_shorter_prefix() {
        let config = GatewayConfig::from_acl(&entrypoint_acl(
            r#"
            hosted_function_service_edge_traffic_intents {
              intents "/functions/sessionless-mcp" {
                traffic {
                  path = "/functions/sessionless-mcp"
                  protocol = "mcp_sessionless"
                  visibility = "public"
                  runtime_port = "http"
                }
                workload {
                  workload_id = "88888888-8888-4888-8888-888888888888"
                  revision_id = "99999999-9999-4999-8999-999999999999"
                  revision_generation = 4
                }
              }
              intents "/functions/sessionless-mcp-b" {
                traffic {
                  path = "/functions/sessionless-mcp-b"
                  protocol = "mcp_sessionless"
                  visibility = "public"
                  runtime_port = "http"
                }
                workload {
                  workload_id = "88888888-8888-4888-8888-888888888888"
                  revision_id = "aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa"
                  revision_generation = 1
                }
                target {
                  unit_id = "workload:b"
                  generation = 1
                  upstream = "http://127.0.0.1:49152/"
                }
              }
            }
            "#,
        ))
        .expect("parse");
        let table = crate::router::RouterTable::from_config(&config.routers).expect("routes");
        let headers = http::HeaderMap::new();
        let longer = table
            .match_request(
                None,
                "/functions/sessionless-mcp-b",
                "GET",
                &headers,
                FUNCTION_ENTRYPOINT,
            )
            .expect("longer path");
        assert!(longer.middlewares.is_empty());
        assert_eq!(
            config.services[&longer.service_name].load_balancer.servers[0].url,
            "http://127.0.0.1:49152/"
        );
        let shorter = table
            .match_request(
                None,
                "/functions/sessionless-mcp",
                "GET",
                &headers,
                FUNCTION_ENTRYPOINT,
            )
            .expect("shorter path");
        assert_eq!(
            shorter.middlewares,
            vec!["hosted-function-0-unavailable".to_string()]
        );
    }

    #[test]
    fn intents_without_the_function_entrypoint_are_rejected() {
        let error = GatewayConfig::from_acl(&intent(
            r#"
            target {
              unit_id = "workload:unit"
              generation = 4
              upstream = "http://127.0.0.1:49152/"
            }
            "#,
        ))
        .expect_err("missing entrypoint");
        assert!(error.to_string().contains("aster-cloud-function"));
    }

    #[test]
    fn profile_concurrency_ceiling_admits_one_request_and_fails_the_next_closed() {
        let config = GatewayConfig::from_acl(&entrypoint_acl(
            r#"
            hosted_function_service_edge_traffic_intents {
              intents "/functions/sessionless-mcp" {
                traffic {
                  path = "/functions/sessionless-mcp"
                  protocol = "mcp_sessionless"
                  visibility = "public"
                  runtime_port = "http"
                }
                workload {
                  workload_id = "88888888-8888-4888-8888-888888888888"
                  revision_id = "99999999-9999-4999-8999-999999999999"
                  revision_generation = 4
                  max_concurrency = 1
                }
                target {
                  unit_id = "workload:unit"
                  generation = 4
                  upstream = "http://127.0.0.1:49152/"
                }
              }
            }
            "#,
        ))
        .expect("parse");
        let service = config
            .services
            .get("hosted-function-0-ceiling-1")
            .expect("ceiling service");
        assert!(service.scaling.is_none());
        let registry =
            crate::service::ServiceRegistry::from_config(&config.services).expect("registry");
        let backend = registry
            .get("hosted-function-0-ceiling-1")
            .expect("load balancer")
            .backends()[0]
            .clone();
        let admitted = backend.admit_connection_on(0);
        assert!(matches!(
            admitted,
            crate::service::ConnectionAdmission::Admitted(_)
        ));
        assert!(matches!(
            backend.admit_connection_on(1),
            crate::service::ConnectionAdmission::ConcurrencyCeiling
        ));
        drop(admitted);
        assert!(matches!(
            backend.admit_connection_on(0),
            crate::service::ConnectionAdmission::Admitted(_)
        ));
    }

    fn scoped_intent(target_and_activation: &str) -> String {
        format!(
            r#"
            hosted_function_service_edge_traffic_intents {{
              intents "/functions/sessionless-mcp" {{
                organization_id = "11111111-1111-4111-8111-111111111111"
                traffic {{
                  path = "/functions/sessionless-mcp"
                  protocol = "mcp_sessionless"
                  visibility = "public"
                  runtime_port = "http"
                }}
                workload {{
                  workload_id = "88888888-8888-4888-8888-888888888888"
                  revision_id = "99999999-9999-4999-8999-999999999999"
                  revision_generation = 4
                }}
                {target_and_activation}
              }}
            }}
            "#
        )
    }

    #[test]
    fn scoped_intent_without_target_records_activation_identity() {
        let config = GatewayConfig::from_acl(&entrypoint_acl(&scoped_intent(""))).expect("parse");
        let activation = &config.hosted_function_activations["hosted-function-0"];
        assert_eq!(
            activation.organization_id,
            uuid::Uuid::parse_str("11111111-1111-4111-8111-111111111111").unwrap()
        );
        assert_eq!(
            activation.workload_id,
            uuid::Uuid::parse_str("88888888-8888-4888-8888-888888888888").unwrap()
        );
        assert_eq!(activation.hold, None);
        assert_eq!(
            activation.retry_after_secs,
            crate::hosted_function_activation::DEFAULT_RETRY_AFTER_SECS
        );
    }

    #[test]
    fn declared_activation_carries_a_bounded_hold() {
        let config = GatewayConfig::from_acl(&entrypoint_acl(&scoped_intent(
            r#"
            activation {
              hold_ms = 15000
              retry_after_s = 3
            }
            "#,
        )))
        .expect("parse");
        let activation = &config.hosted_function_activations["hosted-function-0"];
        assert_eq!(activation.hold, Some(std::time::Duration::from_secs(15)));
        assert_eq!(activation.retry_after_secs, 3);

        let unbounded = GatewayConfig::from_acl(&entrypoint_acl(&scoped_intent(
            "activation {\n hold_ms = 3600000\n }",
        )));
        assert!(unbounded.is_err());
    }

    #[test]
    fn healthy_or_drifted_targets_never_activate() {
        let healthy = GatewayConfig::from_acl(&entrypoint_acl(&scoped_intent(
            r#"
            target {
              unit_id = "workload:unit"
              generation = 4
              upstream = "http://127.0.0.1:49152/"
            }
            activation { hold_ms = 1000 }
            "#,
        )))
        .expect("parse");
        assert!(healthy.hosted_function_activations.is_empty());
        let drifted = GatewayConfig::from_acl(&entrypoint_acl(&scoped_intent(
            r#"
            target {
              unit_id = "workload:unit"
              generation = 3
              upstream = "http://127.0.0.1:49152/"
            }
            activation { hold_ms = 1000 }
            "#,
        )))
        .expect("parse");
        assert!(drifted.hosted_function_activations.is_empty());
    }

    #[test]
    fn a_new_incarnation_of_the_same_revision_is_served() {
        let config = GatewayConfig::from_acl(&entrypoint_acl(&intent(
            r#"
            target {
              unit_id = "workload:unit"
              generation = 9
              revision_id = "99999999-9999-4999-8999-999999999999"
              upstream = "http://127.0.0.1:49152/"
            }
            "#,
        )))
        .expect("parse");
        assert!(config.routers["hosted-function-0"].middlewares.is_empty());
        assert_eq!(
            config.services["hosted-function-0"].load_balancer.servers[0].url,
            "http://127.0.0.1:49152/"
        );
    }

    #[test]
    fn a_target_serving_another_revision_is_drift() {
        let config = GatewayConfig::from_acl(&entrypoint_acl(&intent(
            r#"
            target {
              unit_id = "workload:unit"
              generation = 4
              revision_id = "aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa"
              upstream = "http://127.0.0.1:49152/"
            }
            "#,
        )))
        .expect("parse");
        assert!(config.middlewares["hosted-function-0-unavailable"]
            .value
            .as_deref()
            .unwrap()
            .contains("hosted_function_revision_drift"));
        assert_eq!(
            config.services["hosted-function-0"].load_balancer.servers[0].url,
            FAIL_CLOSED_UPSTREAM
        );
    }

    #[test]
    fn activation_without_cloud_scope_is_rejected() {
        let error =
            GatewayConfig::from_acl(&entrypoint_acl(&intent("activation { hold_ms = 1000 }")))
                .expect_err("unscoped activation");
        assert!(error.to_string().contains("organization_id"));
    }
}
