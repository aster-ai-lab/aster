//! Strict ACL parser for Cloud-managed inference policy.

use super::{bool_attr, children, config_error, type_error, u32_attr, u64_attr};
use crate::config::{
    InferenceConfig, InferenceCredentialConfig, InferenceDistributedServingConfig,
    InferenceEndpoint, InferenceGrantConfig, InferenceLimitsConfig, InferenceModelConfig,
    InferencePhaseRole, InferencePrincipalConfig, InferenceProtocol, InferenceRouteConfig,
    InferenceSchedulingConfig, InferenceTargetConfig, InferenceTransferHealth, InferenceWire,
    InferenceWorkerConfig, ManagedTargetConfig, INFERENCE_TOKENIZER_REVISION,
};
use crate::error::Result;
use a3s_acl::{Block, Value};
use chrono::{DateTime, Utc};
use std::collections::HashMap;
use std::str::FromStr;
use uuid::Uuid;

pub(super) fn parse_inference_block(block: &Block) -> Result<InferenceConfig> {
    ensure_shape(
        block,
        "policy",
        0,
        &["expires_at", "tokenizer_revision"],
        &["credentials", "routes", "workers", "guardrails"],
    )?;
    let expires_at = required_timestamp_attr(block, "expires_at")?;
    let tokenizer_revision = required_literal_string_attr(block, "tokenizer_revision")?;
    if tokenizer_revision != INFERENCE_TOKENIZER_REVISION {
        return Err(config_error(format!(
            "inference tokenizer_revision '{tokenizer_revision}' is not supported; expected '{INFERENCE_TOKENIZER_REVISION}'"
        )));
    }

    let mut credentials = HashMap::new();
    let mut routes = HashMap::new();
    let mut workers = HashMap::new();
    let mut guardrails = HashMap::new();
    for child in &block.blocks {
        match child.name.as_str() {
            "credentials" => {
                let credential = parse_credential(child)?;
                if credentials
                    .insert(credential.credential_id, credential)
                    .is_some()
                {
                    return Err(config_error("Duplicate inference credential ID"));
                }
            }
            "routes" => {
                let route = parse_route(child)?;
                if routes.insert(route.route_id, route).is_some() {
                    return Err(config_error("Duplicate inference route ID"));
                }
            }
            "workers" => {
                let worker = parse_worker(child)?;
                if workers
                    .insert(worker.target.unit_id.clone(), worker)
                    .is_some()
                {
                    return Err(config_error("Duplicate inference worker unit ID"));
                }
            }
            "guardrails" => {
                // The wire feature owns the gate engine; without it the
                // block fails closed rather than parsing into dead policy.
                #[cfg(feature = "wire")]
                {
                    ensure_shape(child, "guardrails", 1, &["sentry_acl"], &[])?;
                    let name = child.labels[0].clone();
                    let sentry_acl = required_literal_string_attr(child, "sentry_acl")?;
                    // Fail closed at parse: an unparsable sentry ACL must
                    // refuse activation, not surface per request.
                    drop(crate::wire::WireGate::from_acl(&sentry_acl)?);
                    let policy = crate::config::InferenceGuardrailsConfig { sentry_acl };
                    if guardrails.insert(name.clone(), policy).is_some() {
                        return Err(config_error(format!(
                            "Duplicate inference guardrails policy '{name}'"
                        )));
                    }
                }
                #[cfg(not(feature = "wire"))]
                {
                    let _ = child;
                    return Err(config_error(
                        "Inference guardrails require a gateway built with the wire feature",
                    ));
                }
            }
            other => {
                return Err(config_error(format!(
                    "Unknown inference policy block '{other}'"
                )))
            }
        }
    }

    // Grants may only reference declared guardrails policies.
    for route in routes.values() {
        for grant in route.grants.values() {
            if let Some(name) = &grant.guardrails {
                if !guardrails.contains_key(name) {
                    return Err(config_error(format!(
                        "Inference grant references unknown guardrails policy '{name}'"
                    )));
                }
            }
        }
    }
    Ok(InferenceConfig {
        expires_at,
        tokenizer_revision,
        credentials,
        routes,
        workers,
        guardrails,
    })
}

fn parse_worker(block: &Block) -> Result<InferenceWorkerConfig> {
    ensure_shape(
        block,
        "worker",
        1,
        &[
            "target_id",
            "generation",
            "schema",
            "worker_epoch",
            "execution_profile_sha256",
            "observation_generation",
            "observed_at",
            "expires_at",
            "phases",
            "prompt_cache_capable",
            "state_transfer_capable",
            "ready_phases",
            "active_limit",
            "active",
            "waiting",
            "prompt_cache_supported",
            "prompt_cache_entries",
            "prompt_cache_capacity",
            "prompt_cache_pressure_basis_points",
            "transfer_health",
            "certified_latency_ms",
        ],
        &[],
    )?;
    let unit_id = block.labels[0].clone();
    Ok(InferenceWorkerConfig {
        target: ManagedTargetConfig {
            target_id: required_uuid_attr(block, "target_id")?,
            unit_id,
            generation: required_u64_attr(block, "generation")?,
        },
        schema: required_literal_string_attr(block, "schema")?,
        worker_epoch: required_uuid_attr(block, "worker_epoch")?,
        execution_profile_sha256: optional_literal_string_attr(block, "execution_profile_sha256")?,
        observation_generation: required_u64_attr(block, "observation_generation")?,
        observed_at: required_timestamp_attr(block, "observed_at")?,
        expires_at: required_timestamp_attr(block, "expires_at")?,
        phases: required_phase_list_attr(block, "phases")?,
        prompt_cache_capable: required_bool_attr(block, "prompt_cache_capable")?,
        state_transfer_capable: required_bool_attr(block, "state_transfer_capable")?,
        ready_phases: required_phase_list_attr(block, "ready_phases")?,
        active_limit: u64_attr(block, &["active_limit"])?,
        active: required_u64_attr(block, "active")?,
        waiting: required_u64_attr(block, "waiting")?,
        prompt_cache_supported: required_bool_attr(block, "prompt_cache_supported")?,
        prompt_cache_entries: required_u64_attr(block, "prompt_cache_entries")?,
        prompt_cache_capacity: required_u64_attr(block, "prompt_cache_capacity")?,
        prompt_cache_pressure_basis_points: required_u16_attr(
            block,
            "prompt_cache_pressure_basis_points",
        )?,
        transfer_health: InferenceTransferHealth::from_str(&required_literal_string_attr(
            block,
            "transfer_health",
        )?)
        .map_err(config_error)?,
        certified_latency_ms: u64_attr(block, &["certified_latency_ms"])?,
    })
}

fn parse_credential(block: &Block) -> Result<InferenceCredentialConfig> {
    ensure_shape(
        block,
        "credential",
        1,
        &[
            "environment_id",
            "audience",
            "prefix",
            "verifier_hash",
            "generation",
            "expires_at",
            "revoked",
        ],
        &[],
    )?;
    Ok(InferenceCredentialConfig {
        credential_id: uuid_label(block, "credential")?,
        environment_id: required_uuid_attr(block, "environment_id")?,
        audience: required_literal_string_attr(block, "audience")?,
        prefix: required_literal_string_attr(block, "prefix")?,
        verifier_hash: required_literal_string_attr(block, "verifier_hash")?,
        generation: required_u64_attr(block, "generation")?,
        expires_at: required_timestamp_attr(block, "expires_at")?,
        revoked: required_bool_attr(block, "revoked")?,
    })
}

fn parse_route(block: &Block) -> Result<InferenceRouteConfig> {
    ensure_shape(
        block,
        "route",
        1,
        &["router", "environment_id", "policy_revision", "wire"],
        &[
            "models",
            "grants",
            "listings",
            "principal",
            "ocr_supplier",
            "decision_supplier",
            "embedding_supplier",
            "rerank_supplier",
            "console_catalog",
            "engine_profiles",
        ],
    )?;
    let route_id = uuid_label(block, "route")?;
    let mut models = HashMap::new();
    let mut grants = HashMap::new();
    let mut listings = HashMap::new();
    let mut engine_profiles = Vec::new();
    let mut principal = None;
    let mut ocr_supplier = None;
    let mut decision_supplier = None;
    let mut embedding_supplier = None;
    let mut rerank_supplier = None;
    let mut console_catalog = None;
    for child in &block.blocks {
        match child.name.as_str() {
            "principal" => {
                if principal.replace(parse_principal(child)?).is_some() {
                    return Err(config_error(
                        "Inference route defines more than one principal block",
                    ));
                }
            }
            "listings" => {
                let (sku, listing) = parse_listing(child)?;
                if listings.insert(sku.clone(), listing).is_some() {
                    return Err(config_error(format!(
                        "Duplicate inference listing sku '{sku}'"
                    )));
                }
            }
            "models" => {
                let (alias, model) = parse_model(child)?;
                if models.insert(alias.clone(), model).is_some() {
                    return Err(config_error(format!(
                        "Duplicate inference model alias '{alias}'"
                    )));
                }
            }
            "grants" => {
                let credential_id = uuid_label(child, "grant credential")?;
                let grant = parse_grant(child)?;
                if grants.insert(credential_id, grant).is_some() {
                    return Err(config_error(format!(
                        "Duplicate inference grant credential ID {credential_id}"
                    )));
                }
            }
            "ocr_supplier" => {
                if ocr_supplier
                    .replace(parse_supplier(child, "ocr_supplier")?)
                    .is_some()
                {
                    return Err(config_error(
                        "Inference route defines more than one ocr_supplier block",
                    ));
                }
            }
            "decision_supplier" => {
                if decision_supplier
                    .replace(parse_supplier(child, "decision_supplier")?)
                    .is_some()
                {
                    return Err(config_error(
                        "Inference route defines more than one decision_supplier block",
                    ));
                }
            }
            "embedding_supplier" => {
                if embedding_supplier
                    .replace(parse_supplier(child, "embedding_supplier")?)
                    .is_some()
                {
                    return Err(config_error(
                        "Inference route defines more than one embedding_supplier block",
                    ));
                }
            }
            "rerank_supplier" => {
                if rerank_supplier
                    .replace(parse_supplier(child, "rerank_supplier")?)
                    .is_some()
                {
                    return Err(config_error(
                        "Inference route defines more than one rerank_supplier block",
                    ));
                }
            }
            "engine_profiles" => {
                engine_profiles.push(parse_engine_profile(child)?);
            }
            "console_catalog" => {
                if console_catalog
                    .replace(parse_console_catalog(child)?)
                    .is_some()
                {
                    return Err(config_error(
                        "Inference route defines more than one console_catalog block",
                    ));
                }
            }
            other => {
                return Err(config_error(format!(
                    "Unknown inference route block '{other}'"
                )))
            }
        }
    }

    ensure_budgeted_grants_cover_priced_models(&models, &grants, principal.as_ref())?;

    Ok(InferenceRouteConfig {
        route_id,
        router: required_literal_string_attr(block, "router")?,
        environment_id: required_uuid_attr(block, "environment_id")?,
        policy_revision: required_u64_attr(block, "policy_revision")?,
        models,
        grants,
        listings,
        wire: optional_literal_string_attr(block, "wire")?
            .map(|wire| InferenceWire::from_str(&wire).map_err(config_error))
            .transpose()?
            .unwrap_or_default(),
        principal,
        ocr_supplier,
        decision_supplier,
        embedding_supplier,
        rerank_supplier,
        console_catalog,
        engine_profiles,
    })
}

/// GE0: the engine_profiles block — labeled by name, attrs for the
/// scalar fields, list attrs for args/environment_keys, a nested
/// accelerator block. Fail-closed on shape.
fn parse_engine_profile(block: &Block) -> Result<crate::config::inference::EngineProfileConfig> {
    use crate::config::inference::{EngineAcceleratorShapeConfig, EngineProfileConfig};
    ensure_shape(
        block,
        "engine_profiles",
        1,
        &[
            "engine",
            "image",
            "health_path",
            "port",
            "args",
            "environment_keys",
        ],
        &["accelerator"],
    )?;
    let mut accelerator = None;
    for child in &block.blocks {
        match child.name.as_str() {
            "accelerator" => {
                ensure_shape(child, "accelerator", 0, &["count", "vram_mib", "kind"], &[])?;
                accelerator = Some(EngineAcceleratorShapeConfig {
                    count: u32_attr(child, &["count"])?
                        .ok_or_else(|| config_error("engine profile accelerator requires count"))?,
                    vram_mib: u32_attr(child, &["vram_mib"])?.ok_or_else(|| {
                        config_error("engine profile accelerator requires vram_mib")
                    })?,
                    kind: required_literal_string_attr(child, "kind")?,
                });
            }
            other => {
                return Err(config_error(format!(
                    "Unknown engine_profiles block '{other}'"
                )))
            }
        }
    }
    let profile = EngineProfileConfig {
        name: block.labels[0].clone(),
        engine: required_literal_string_attr(block, "engine")?,
        image: required_literal_string_attr(block, "image")?,
        args_template: required_literal_string_list_attr(block, "args")?,
        accelerator: accelerator
            .ok_or_else(|| config_error("engine profile requires an accelerator block"))?,
        health_path: required_literal_string_attr(block, "health_path")?,
        port: u16::try_from(
            u32_attr(block, &["port"])?
                .ok_or_else(|| config_error("engine profile requires port"))?,
        )
        .map_err(|_| type_error("port", "u16"))?,
        environment_keys: optional_literal_string_list_attr(block, "environment_keys")?,
    };
    profile.validate().map_err(config_error)?;
    Ok(profile)
}

fn optional_poll_interval(block: &Block) -> Result<Option<u64>> {
    u64_attr(block, &["poll_interval"])
}

fn parse_console_catalog(block: &Block) -> Result<crate::config::ConsoleCatalogConfig> {
    ensure_shape(
        block,
        "console_catalog",
        1,
        &[
            "api_service",
            "poll_interval",
            "prefix_affinity",
            "excluded_supplier_ids",
            "prefix_parallelism",
        ],
        &[],
    )?;
    Ok(crate::config::ConsoleCatalogConfig {
        prefix_parallelism: optional_prefix_parallelism(block)?,
        excluded_supplier_ids: {
            let ids = optional_literal_string_list_attr(block, "excluded_supplier_ids")?;
            if ids.len() > 256
                || ids
                    .iter()
                    .any(|id| id.is_empty() || id.len() > 256 || id.trim() != id)
            {
                return Err(config_error("console_catalog excluded_supplier_ids requires at most 256 exact nonempty IDs of at most 256 bytes"));
            }
            ids
        },
        prefix_affinity: bool_attr(block, &["prefix_affinity"])?.unwrap_or(false),
        api_service: required_literal_string_attr(block, "api_service")?,
        poll_interval_secs: optional_poll_interval(block)?
            .unwrap_or(crate::config::default_catalog_poll_interval()),
    })
}

fn parse_supplier(block: &Block, name: &str) -> Result<InferenceTargetConfig> {
    ensure_shape(
        block,
        name,
        1,
        &[
            "service",
            "upstream_model",
            "priority",
            "weight",
            "protocol",
            "api_key_env",
        ],
        &[],
    )?;
    Ok(InferenceTargetConfig {
        target_id: uuid_label(block, name)?,
        service: required_literal_string_attr(block, "service")?,
        upstream_model: required_literal_string_attr(block, "upstream_model")?,
        priority: required_u32_attr(block, "priority")?,
        weight: required_u32_attr(block, "weight")?,
        protocol: InferenceProtocol::from_str(&required_literal_string_attr(block, "protocol")?)
            .map_err(config_error)?,
        api_key_env: optional_literal_string_attr(block, "api_key_env")?,
        translate: false,
    })
}

fn parse_principal(block: &Block) -> Result<InferencePrincipalConfig> {
    ensure_shape(
        block,
        "principal",
        0,
        &[
            "verify_url",
            "subject_header",
            "org_header",
            "cache_ttl_secs",
            "models",
            "endpoints",
        ],
        &["limits"],
    )?;
    let limits = children(block, &["limits"]);
    if limits.len() != 1 {
        return Err(config_error(
            "Inference principal requires exactly one limits block",
        ));
    }
    Ok(InferencePrincipalConfig {
        verify_url: required_literal_string_attr(block, "verify_url")?,
        subject_header: optional_literal_string_attr(block, "subject_header")?
            .unwrap_or_else(|| "x-a3s-subject".to_string()),
        org_header: optional_literal_string_attr(block, "org_header")?,
        cache_ttl_secs: u64_attr(block, &["cache_ttl_secs"])?.unwrap_or(60),
        models: optional_literal_string_list_attr(block, "models")?,
        endpoints: parse_endpoints(block)?,
        limits: parse_limits(limits[0])?,
    })
}

/// HR-cloud: the task-routing plan block (the HiRoute alignment).
/// Groups name declared target ids — validated against the model's
/// targets after parse.
fn parse_routing(block: &Block) -> Result<crate::config::inference::InferenceRoutingPlan> {
    use crate::config::inference::{
        InferenceRoutingBranch, InferenceRoutingMode, InferenceRoutingPlan,
    };
    ensure_shape(
        block,
        "routing",
        0,
        &[
            "mode",
            "decision_model",
            "economy_targets",
            "primary_targets",
            "simple_probability_threshold_bp",
            "competence_floor_bp",
        ],
        &["branches"],
    )?;
    let mode = match required_literal_string_attr(block, "mode")?.as_str() {
        "fixed" => InferenceRoutingMode::Fixed,
        "smart_saving" => InferenceRoutingMode::SmartSaving,
        "branches" => InferenceRoutingMode::Branches,
        "free_first" => InferenceRoutingMode::FreeFirst,
        other => {
            return Err(config_error(format!(
                "routing mode '{other}' is not one of fixed, smart_saving, branches, free_first"
            )))
        }
    };
    let uuid_list = |key: &str| uuid_list_on(block, key);
    let mut branches = Vec::new();
    for child in &block.blocks {
        if child.name != "branches" {
            return Err(config_error(format!(
                "Unknown routing block '{}'",
                child.name
            )));
        }
        ensure_shape(
            child,
            "branches",
            1,
            &["regular_targets", "primary_targets"],
            &[],
        )?;
        let category = child.labels[0].clone();
        branches.push(InferenceRoutingBranch {
            category,
            regular_targets: uuid_list_on(child, "regular_targets")?
                .ok_or_else(|| config_error("branches requires regular_targets"))?,
            primary_targets: uuid_list_on(child, "primary_targets")?,
        });
    }
    let plan = InferenceRoutingPlan {
        mode,
        decision_model: optional_literal_string_attr(block, "decision_model")?,
        economy_targets: uuid_list("economy_targets")?,
        primary_targets: uuid_list("primary_targets")?,
        simple_probability_threshold_bp: u32_attr(block, &["simple_probability_threshold_bp"])?
            .unwrap_or(8000),
        competence_floor_bp: u32_attr(block, &["competence_floor_bp"])?.unwrap_or(5000),
        branches,
    };
    Ok(plan)
}

fn uuid_list_on(block: &Block, key: &str) -> crate::error::Result<Option<Vec<uuid::Uuid>>> {
    match block.attributes.get(key) {
        None => Ok(None),
        Some(a3s_acl::Value::List(values)) => values
            .iter()
            .map(|value| match value {
                a3s_acl::Value::String(text) => text
                    .parse::<uuid::Uuid>()
                    .map_err(|_| type_error(key, "list of uuids")),
                _ => Err(type_error(key, "list of uuids")),
            })
            .collect::<crate::error::Result<Vec<_>>>()
            .map(Some),
        Some(_) => Err(type_error(key, "list of uuids")),
    }
}

fn parse_endpoints(block: &Block) -> Result<Vec<InferenceEndpoint>> {
    required_literal_string_list_attr(block, "endpoints")?
        .into_iter()
        .map(|endpoint| InferenceEndpoint::from_str(&endpoint).map_err(config_error))
        .collect()
}

fn parse_listing(block: &Block) -> Result<(String, crate::config::InferenceListingConfig)> {
    ensure_shape(block, "listing", 1, &["listing_id", "service"], &[])?;
    Ok((
        block.labels[0].clone(),
        crate::config::InferenceListingConfig {
            listing_id: required_uuid_attr(block, "listing_id")?,
            service: required_literal_string_attr(block, "service")?,
        },
    ))
}

fn parse_model(block: &Block) -> Result<(String, InferenceModelConfig)> {
    ensure_shape(
        block,
        "model",
        1,
        &[
            "model_id",
            "latency_bias",
            "prefix_parallelism",
            "prefix_affinity",
            "engine_profile",
        ],
        &["targets", "scheduling", "pricing", "routing"],
    )?;
    let alias = block.labels[0].clone();
    let mut targets = Vec::new();
    let mut scheduling = None;
    let mut pricing = None;
    let mut routing = None;
    for child in &block.blocks {
        match child.name.as_str() {
            "targets" => targets.push(parse_target(child)?),
            "scheduling" => {
                if scheduling.replace(parse_scheduling(child)?).is_some() {
                    return Err(config_error(format!(
                        "Inference model '{alias}' defines more than one scheduling block"
                    )));
                }
            }
            "pricing" => {
                if pricing.replace(parse_pricing(child)?).is_some() {
                    return Err(config_error(format!(
                        "Inference model '{alias}' defines more than one pricing block"
                    )));
                }
            }
            "routing" => {
                if routing.replace(parse_routing(child)?).is_some() {
                    return Err(config_error(format!(
                        "Inference model '{alias}' defines more than one routing block"
                    )));
                }
            }
            other => {
                return Err(config_error(format!(
                    "Unknown inference model block '{other}'"
                )))
            }
        }
    }
    Ok((
        alias,
        InferenceModelConfig {
            prefix_parallelism: optional_prefix_parallelism(block)?,
            routing,
            engine_profile: optional_literal_string_attr(block, "engine_profile")?,
            model_id: required_uuid_attr(block, "model_id")?,
            failover: Default::default(),
            pricing,
            latency_bias: bool_attr(block, &["latency_bias"])?.unwrap_or(false),
            prefix_affinity: bool_attr(block, &["prefix_affinity"])?.unwrap_or(false),
            targets,
            scheduling,
        },
    ))
}

/// Per-model serving price block: micro-USD per one million tokens.
fn parse_pricing(block: &Block) -> Result<crate::config::InferenceModelPricing> {
    ensure_shape(
        block,
        "pricing",
        0,
        &["input_micros_per_million", "output_micros_per_million"],
        &[],
    )?;
    Ok(crate::config::InferenceModelPricing {
        input_micros_per_million: required_u64_attr(block, "input_micros_per_million")?,
        output_micros_per_million: required_u64_attr(block, "output_micros_per_million")?,
    })
}

fn parse_scheduling(block: &Block) -> Result<InferenceSchedulingConfig> {
    ensure_shape(
        block,
        "scheduling",
        0,
        &[
            "phase",
            "max_concurrent_requests",
            "max_queued_requests",
            "queue_timeout_ms",
            "prompt_cache_affinity",
        ],
        &["distributed_serving"],
    )?;
    let distributed_serving = children(block, &["distributed_serving"]);
    if distributed_serving.len() > 1 {
        return Err(config_error(
            "Inference scheduling defines more than one distributed_serving block",
        ));
    }
    Ok(InferenceSchedulingConfig {
        phase: InferencePhaseRole::from_str(&required_literal_string_attr(block, "phase")?)
            .map_err(config_error)?,
        max_concurrent_requests: required_u64_attr(block, "max_concurrent_requests")?,
        max_queued_requests: required_u64_attr(block, "max_queued_requests")?,
        queue_timeout_ms: required_u64_attr(block, "queue_timeout_ms")?,
        prompt_cache_affinity: required_bool_attr(block, "prompt_cache_affinity")?,
        distributed_serving: distributed_serving
            .first()
            .map(|block| parse_distributed_serving(block))
            .transpose()?,
    })
}

fn parse_distributed_serving(block: &Block) -> Result<InferenceDistributedServingConfig> {
    ensure_shape(
        block,
        "distributed_serving",
        0,
        &["api_key_env", "execution_timeout_ms"],
        &[],
    )?;
    Ok(InferenceDistributedServingConfig {
        api_key_env: required_literal_string_attr(block, "api_key_env")?,
        execution_timeout_ms: required_u64_attr(block, "execution_timeout_ms")?,
    })
}

fn parse_target(block: &Block) -> Result<InferenceTargetConfig> {
    ensure_shape(
        block,
        "target",
        1,
        &[
            "service",
            "upstream_model",
            "priority",
            "weight",
            "protocol",
            "api_key_env",
            "translate",
        ],
        &[],
    )?;
    Ok(InferenceTargetConfig {
        target_id: uuid_label(block, "target")?,
        service: required_literal_string_attr(block, "service")?,
        upstream_model: required_literal_string_attr(block, "upstream_model")?,
        priority: required_u32_attr(block, "priority")?,
        weight: required_u32_attr(block, "weight")?,
        protocol: optional_literal_string_attr(block, "protocol")?
            .map(|protocol| InferenceProtocol::from_str(&protocol).map_err(config_error))
            .transpose()?
            .unwrap_or_default(),
        api_key_env: optional_literal_string_attr(block, "api_key_env")?,
        translate: bool_attr(block, &["translate"])?.unwrap_or(false),
    })
}

fn parse_grant(block: &Block) -> Result<InferenceGrantConfig> {
    ensure_shape(
        block,
        "grant",
        1,
        &[
            "credential_generation",
            "models",
            "endpoints",
            "listings",
            "guardrails",
        ],
        &["limits"],
    )?;
    let limits = children(block, &["limits"]);
    if limits.len() != 1 {
        return Err(config_error(
            "Inference grant requires exactly one limits block",
        ));
    }
    let endpoints = parse_endpoints(block)?;
    Ok(InferenceGrantConfig {
        credential_generation: required_u64_attr(block, "credential_generation")?,
        models: required_literal_string_list_attr(block, "models")?,
        endpoints,
        listings: optional_literal_string_list_attr(block, "listings")?,
        guardrails: optional_literal_string_attr(block, "guardrails")?,
        limits: parse_limits(limits[0])?,
    })
}

fn parse_limits(block: &Block) -> Result<InferenceLimitsConfig> {
    ensure_shape(
        block,
        "limits",
        0,
        &[
            "max_concurrent_requests",
            "requests_per_minute",
            "request_burst",
            "tokens_per_minute",
            "max_spend_micros",
            "spend_window",
            "spend_baseline_micros",
            "baseline_epoch",
        ],
        &[],
    )?;
    let max_spend_micros = u64_attr(block, &["max_spend_micros"])?;
    let spend_window = optional_literal_string_attr(block, "spend_window")?
        .map(|window| parse_spend_window(&window))
        .transpose()?;
    match (max_spend_micros, spend_window) {
        (Some(_), Some(_)) | (None, None) => {}
        (Some(_), None) => {
            return Err(config_error(
                "Inference limits with max_spend_micros require spend_window",
            ))
        }
        (None, Some(_)) => {
            return Err(config_error(
                "Inference limits spend_window requires max_spend_micros",
            ))
        }
    }
    let spend_baseline_micros = u64_attr(block, &["spend_baseline_micros"])?;
    let baseline_epoch = u64_attr(block, &["baseline_epoch"])?;
    if spend_baseline_micros.is_some() && max_spend_micros.is_none() {
        return Err(config_error(
            "Inference limits spend_baseline_micros require max_spend_micros",
        ));
    }
    if baseline_epoch.is_some() && spend_baseline_micros.is_none() {
        return Err(config_error(
            "Inference limits baseline_epoch requires spend_baseline_micros",
        ));
    }
    if let (Some(baseline), Some(max)) = (spend_baseline_micros, max_spend_micros) {
        if baseline > max {
            return Err(config_error(
                "Inference limits spend_baseline_micros must not exceed max_spend_micros",
            ));
        }
    }
    Ok(InferenceLimitsConfig {
        max_concurrent_requests: required_u64_attr(block, "max_concurrent_requests")?,
        requests_per_minute: required_u64_attr(block, "requests_per_minute")?,
        request_burst: required_u64_attr(block, "request_burst")?,
        tokens_per_minute: required_u64_attr(block, "tokens_per_minute")?,
        max_spend_micros,
        spend_window,
        spend_baseline_micros,
        baseline_epoch,
    })
}

fn parse_spend_window(value: &str) -> Result<crate::config::InferenceSpendWindow> {
    match value {
        "minute" => Ok(crate::config::InferenceSpendWindow::Minute),
        "hour" => Ok(crate::config::InferenceSpendWindow::Hour),
        "day" => Ok(crate::config::InferenceSpendWindow::Day),
        "month" => Ok(crate::config::InferenceSpendWindow::Month),
        other => Err(config_error(format!(
            "Unknown inference spend_window '{other}', expected one of minute, hour, day, month"
        ))),
    }
}

/// A grant with a local spend budget may only cover priced models: an
/// unpriced model under a budget would pass free, the wrong direction.
fn ensure_budgeted_grants_cover_priced_models(
    models: &HashMap<String, InferenceModelConfig>,
    grants: &HashMap<Uuid, InferenceGrantConfig>,
    principal: Option<&InferencePrincipalConfig>,
) -> Result<()> {
    for (credential_id, grant) in grants {
        if grant.limits.max_spend_micros.is_none() {
            continue;
        }
        for alias in &grant.models {
            match models.get(alias) {
                Some(model) if model.pricing.is_some() => {}
                Some(_) => {
                    return Err(config_error(format!(
                        "Inference grant {credential_id} carries a spend budget but model '{alias}' declares no pricing"
                    )))
                }
                None => {
                    return Err(config_error(format!(
                        "Inference grant {credential_id} carries a spend budget but model '{alias}' is not declared"
                    )))
                }
            }
        }
    }
    if let Some(principal) = principal {
        if principal.limits.max_spend_micros.is_some() {
            let covers_all = principal.models.iter().any(|alias| alias == "*");
            for (alias, model) in models {
                let covered = covers_all || principal.models.iter().any(|granted| granted == alias);
                if covered && model.pricing.is_none() {
                    return Err(config_error(format!(
                        "Inference principal carries a spend budget but model '{alias}' declares no pricing"
                    )));
                }
            }
        }
    }
    Ok(())
}

fn ensure_shape(
    block: &Block,
    context: &str,
    label_count: usize,
    allowed_attributes: &[&str],
    allowed_blocks: &[&str],
) -> Result<()> {
    if block.labels.len() != label_count {
        return Err(config_error(format!(
            "Inference {context} block requires exactly {label_count} label(s)"
        )));
    }
    for attribute in block.attributes.keys() {
        if !allowed_attributes.contains(&attribute.as_str()) {
            return Err(config_error(format!(
                "Unknown inference {context} field '{attribute}'"
            )));
        }
    }
    for child in &block.blocks {
        if !allowed_blocks.contains(&child.name.as_str()) {
            return Err(config_error(format!(
                "Unknown inference {context} block '{}'",
                child.name
            )));
        }
    }
    Ok(())
}

fn uuid_label(block: &Block, context: &str) -> Result<Uuid> {
    Uuid::parse_str(&block.labels[0]).map_err(|error| {
        config_error(format!(
            "Invalid inference {context} ID '{}': {error}",
            block.labels[0]
        ))
    })
}

fn required_literal_string_attr(block: &Block, key: &str) -> Result<String> {
    match block.attributes.get(key) {
        Some(Value::String(value)) => Ok(value.clone()),
        Some(_) => Err(type_error(key, "literal string")),
        None => Err(config_error(format!("{} block requires {key}", block.name))),
    }
}

fn optional_literal_string_attr(block: &Block, key: &str) -> Result<Option<String>> {
    match block.attributes.get(key) {
        Some(Value::String(value)) => Ok(Some(value.clone())),
        Some(_) => Err(type_error(key, "literal string")),
        None => Ok(None),
    }
}

fn required_literal_string_list_attr(block: &Block, key: &str) -> Result<Vec<String>> {
    match block.attributes.get(key) {
        Some(Value::List(values)) => values
            .iter()
            .map(|value| match value {
                Value::String(value) => Ok(value.clone()),
                _ => Err(type_error(key, "list of literal strings")),
            })
            .collect(),
        Some(Value::String(value)) => Ok(vec![value.clone()]),
        Some(_) => Err(type_error(key, "list of literal strings")),
        None => Err(config_error(format!("{} block requires {key}", block.name))),
    }
}

fn optional_literal_string_list_attr(block: &Block, key: &str) -> Result<Vec<String>> {
    match block.attributes.get(key) {
        None => Ok(Vec::new()),
        Some(Value::List(values)) => values
            .iter()
            .map(|value| match value {
                Value::String(value) => Ok(value.clone()),
                _ => Err(type_error(key, "list of literal strings")),
            })
            .collect(),
        Some(Value::String(value)) => Ok(vec![value.clone()]),
        Some(_) => Err(type_error(key, "list of literal strings")),
    }
}

fn required_uuid_attr(block: &Block, key: &str) -> Result<Uuid> {
    let value = required_literal_string_attr(block, key)?;
    Uuid::parse_str(&value)
        .map_err(|error| config_error(format!("Invalid inference {key} '{value}': {error}")))
}

fn required_timestamp_attr(block: &Block, key: &str) -> Result<DateTime<Utc>> {
    let value = required_literal_string_attr(block, key)?;
    DateTime::parse_from_rfc3339(&value)
        .map(|timestamp| timestamp.with_timezone(&Utc))
        .map_err(|error| config_error(format!("Invalid inference {key} '{value}': {error}")))
}

fn required_bool_attr(block: &Block, key: &str) -> Result<bool> {
    bool_attr(block, &[key])?
        .ok_or_else(|| config_error(format!("{} block requires {key}", block.name)))
}

fn required_u32_attr(block: &Block, key: &str) -> Result<u32> {
    u32_attr(block, &[key])?
        .ok_or_else(|| config_error(format!("{} block requires {key}", block.name)))
}

fn required_u16_attr(block: &Block, key: &str) -> Result<u16> {
    let value = required_u64_attr(block, key)?;
    u16::try_from(value).map_err(|_| config_error(format!("Inference {key} exceeds the u16 range")))
}

fn required_phase_list_attr(block: &Block, key: &str) -> Result<Vec<InferencePhaseRole>> {
    required_literal_string_list_attr(block, key)?
        .into_iter()
        .map(|phase| InferencePhaseRole::from_str(&phase).map_err(config_error))
        .collect()
}

fn required_u64_attr(block: &Block, key: &str) -> Result<u64> {
    u64_attr(block, &[key])?
        .ok_or_else(|| config_error(format!("{} block requires {key}", block.name)))
}

fn optional_prefix_parallelism(block: &Block) -> Result<u32> {
    let slots = u32_attr(block, &["prefix_parallelism"])?.unwrap_or(1);
    if !(1..=1024).contains(&slots) {
        return Err(config_error("prefix_parallelism must be in 1..=1024"));
    }
    Ok(slots)
}
