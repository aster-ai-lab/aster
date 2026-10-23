//! Referential, security, and resource-bound validation for inference policy.

mod scheduling;

use scheduling::{validate_scheduled_target, validate_scheduling, validate_worker};

use super::{
    InferenceConfig, InferenceCredentialConfig, InferenceEndpoint, InferenceGrantConfig,
    InferenceLimitsConfig, InferenceModelConfig, InferencePrincipalConfig, InferenceProtocol,
    InferenceRouteConfig, InferenceTargetConfig, InferenceWorkerConfig,
    INFERENCE_CREDENTIAL_AUDIENCE, INFERENCE_TOKENIZER_REVISION,
};
use crate::config::{GatewayConfig, OperatingMode};
use crate::error::{GatewayError, Result};
use crate::inference::valid_model_alias;
use argon2::password_hash::PasswordHash;
use chrono::{DateTime, Utc};
use std::collections::{BTreeSet, HashMap, HashSet};
use uuid::Uuid;

const MAX_CREDENTIALS: usize = 10_000;
const MAX_ROUTES: usize = 1_000;
const MAX_MODELS_PER_ROUTE: usize = 1_000;
const MAX_GRANTS_PER_ROUTE: usize = 10_000;
const MAX_TARGETS_PER_MODEL: usize = 32;
const MAX_WORKERS: usize = 10_000;
const MAX_VERIFIER_BYTES: usize = 512;
const MIN_ARGON2_MEMORY_KIB: u32 = 19_456;
const MAX_ARGON2_MEMORY_KIB: u32 = 262_144;
const MIN_ARGON2_ITERATIONS: u32 = 2;
const MAX_ARGON2_ITERATIONS: u32 = 10;
const MAX_ARGON2_LANES: u32 = 4;
const MIN_ARGON2_SALT_ENCODED_LEN: usize = 22;
const MAX_ARGON2_SALT_ENCODED_LEN: usize = 86;
const MIN_ARGON2_OUTPUT_ENCODED_LEN: usize = 43;
const MAX_ARGON2_OUTPUT_ENCODED_LEN: usize = 86;

impl InferenceConfig {
    pub(in crate::config) fn validate(
        &self,
        gateway: &GatewayConfig,
        now: DateTime<Utc>,
    ) -> Result<()> {
        let cloud_issued = !self.credentials.is_empty()
            || !self.workers.is_empty()
            || self
                .routes
                .values()
                .any(|route| route.principal.is_none() || !route.grants.is_empty());
        if cloud_issued
            && (gateway.mode != OperatingMode::CloudManaged || gateway.managed.gateway_id.is_none())
        {
            return Err(config_error(
                "inference policy requires cloud-managed mode with managed.gateway_id unless every route is principal-only",
            ));
        }
        if self.expires_at <= now {
            return Err(config_error("inference policy has expired"));
        }
        if self.tokenizer_revision != INFERENCE_TOKENIZER_REVISION {
            return Err(config_error(format!(
                "inference tokenizer_revision '{}' is not supported; expected '{INFERENCE_TOKENIZER_REVISION}'",
                self.tokenizer_revision
            )));
        }
        if self.credentials.len() > MAX_CREDENTIALS {
            return Err(config_error(format!(
                "inference policy exceeds the {MAX_CREDENTIALS} credential limit"
            )));
        }
        for route in self.routes.values() {
            for grant in route.grants.values() {
                if let Some(name) = &grant.guardrails {
                    if !self.guardrails.contains_key(name) {
                        return Err(config_error(format!(
                            "inference grant references unknown guardrails policy '{name}'"
                        )));
                    }
                }
            }
        }
        if self.routes.len() > MAX_ROUTES {
            return Err(config_error(format!(
                "inference policy exceeds the {MAX_ROUTES} route limit"
            )));
        }
        if self.workers.len() > MAX_WORKERS {
            return Err(config_error(format!(
                "inference policy exceeds the {MAX_WORKERS} worker limit"
            )));
        }

        for (unit_id, worker) in &self.workers {
            if unit_id != &worker.target.unit_id {
                return Err(config_error(format!(
                    "inference worker map key '{unit_id}' does not match unit_id '{}'",
                    worker.target.unit_id
                )));
            }
            validate_worker(worker, self.expires_at, now)?;
        }

        let mut prefixes = HashSet::new();
        for (credential_id, credential) in &self.credentials {
            if *credential_id != credential.credential_id {
                return Err(config_error(format!(
                    "inference credential map key {credential_id} does not match credential_id {}",
                    credential.credential_id
                )));
            }
            validate_credential(credential)?;
            if !prefixes.insert(credential.prefix.as_str()) {
                return Err(config_error(format!(
                    "inference credential prefix '{}' is not unique",
                    credential.prefix
                )));
            }
        }
        let mut ordered_prefixes = prefixes.into_iter().collect::<Vec<_>>();
        ordered_prefixes.sort_unstable();
        if ordered_prefixes
            .windows(2)
            .any(|pair| pair[1].starts_with(pair[0]))
        {
            return Err(config_error(
                "inference credential prefixes must not overlap",
            ));
        }

        let mut routers = HashSet::new();
        // unit_id -> target_id. The same Power observation may back multiple
        // aggregated model aliases that share one managed target (chat+embed).
        let mut scheduled_workers = HashMap::<String, Uuid>::new();
        for (route_id, route) in &self.routes {
            if *route_id != route.route_id {
                return Err(config_error(format!(
                    "inference route map key {route_id} does not match route_id {}",
                    route.route_id
                )));
            }
            validate_route(
                route,
                gateway,
                &self.credentials,
                &mut routers,
                &self.workers,
                &mut scheduled_workers,
            )?;
        }
        if scheduled_workers.len() != self.workers.len() {
            let orphan = self
                .workers
                .keys()
                .find(|unit_id| !scheduled_workers.contains_key(unit_id.as_str()))
                .map(String::as_str)
                .unwrap_or("unknown");
            return Err(config_error(format!(
                "inference worker '{orphan}' is not bound to a scheduled model endpoint"
            )));
        }

        Ok(())
    }

    pub(crate) fn validate_managed_expiry(
        &self,
        managed_expires_at: DateTime<Utc>,
    ) -> std::result::Result<(), String> {
        if self.expires_at != managed_expires_at {
            return Err(
                "Inference policy expires_at must exactly match the managed snapshot expiry"
                    .to_string(),
            );
        }
        Ok(())
    }
}

fn validate_credential(credential: &InferenceCredentialConfig) -> Result<()> {
    if credential.credential_id.is_nil() || credential.environment_id.is_nil() {
        return Err(config_error(
            "inference credential and environment IDs must not be nil",
        ));
    }
    if credential.audience != INFERENCE_CREDENTIAL_AUDIENCE {
        return Err(config_error(format!(
            "inference credential {} has unsupported audience '{}'",
            credential.credential_id, credential.audience
        )));
    }
    if credential.generation == 0 {
        return Err(config_error(format!(
            "inference credential {} generation must be positive",
            credential.credential_id
        )));
    }
    validate_credential_prefix(&credential.prefix)?;
    validate_argon2id_verifier(credential.verifier_hash())?;
    Ok(())
}

fn validate_route<'a>(
    route: &'a InferenceRouteConfig,
    gateway: &'a GatewayConfig,
    credentials: &'a HashMap<Uuid, InferenceCredentialConfig>,
    routers: &mut HashSet<&'a str>,
    workers: &HashMap<String, InferenceWorkerConfig>,
    scheduled_workers: &mut HashMap<String, Uuid>,
) -> Result<()> {
    if route.route_id.is_nil() || route.environment_id.is_nil() {
        return Err(config_error(
            "inference route and environment IDs must not be nil",
        ));
    }
    if route.policy_revision == 0 {
        return Err(config_error(format!(
            "inference route {} policy_revision must be positive",
            route.route_id
        )));
    }
    if route.router.is_empty()
        || route.router.len() > 255
        || route.router.trim() != route.router
        || route.router.chars().any(char::is_control)
    {
        return Err(config_error(format!(
            "inference route {} has an invalid router name",
            route.route_id
        )));
    }
    if !gateway.routers.contains_key(&route.router) {
        return Err(config_error(format!(
            "inference route {} references unknown router '{}'",
            route.route_id, route.router
        )));
    }
    if let Some((entrypoint, protocol)) =
        super::super::router_non_http_binding(&gateway.entrypoints, &gateway.routers[&route.router])
    {
        return Err(config_error(format!(
            "inference route {} binds router '{}', which applies only to protocol http (entrypoint '{entrypoint}' is {})",
            route.route_id,
            route.router,
            super::super::listener_protocol_name(protocol)
        )));
    }
    if !routers.insert(route.router.as_str()) {
        return Err(config_error(format!(
            "Gateway router '{}' is bound to more than one inference route",
            route.router
        )));
    }
    // A console-catalog route starts empty: the poller fills its models at
    // runtime, so only the upper bound applies to it.
    let catalog_supplied = route.console_catalog.is_some();
    if (route.models.is_empty() && !catalog_supplied) || route.models.len() > MAX_MODELS_PER_ROUTE {
        return Err(config_error(format!(
            "inference route {} must contain 1 to {MAX_MODELS_PER_ROUTE} models",
            route.route_id
        )));
    }
    if route.grants.len() > MAX_GRANTS_PER_ROUTE {
        return Err(config_error(format!(
            "inference route {} exceeds the {MAX_GRANTS_PER_ROUTE} grant limit",
            route.route_id
        )));
    }

    if let Some(catalog) = &route.console_catalog {
        if !(1..=1024).contains(&catalog.prefix_parallelism) {
            return Err(config_error("prefix_parallelism must be in 1..=1024"));
        }
        if catalog.excluded_supplier_ids.len() > 256
            || catalog
                .excluded_supplier_ids
                .iter()
                .any(|id| id.is_empty() || id.len() > 256 || id.trim() != id)
        {
            return Err(config_error("console_catalog excluded_supplier_ids requires at most 256 exact nonempty IDs of at most 256 bytes"));
        }
        if !route.models.is_empty() {
            return Err(config_error(format!(
                "inference route {} declares both models and console_catalog — one source per router",
                route.route_id
            )));
        }
        if !gateway.services.contains_key(&catalog.api_service) {
            return Err(config_error(format!(
                "inference route {} console_catalog references unknown service '{}'",
                route.route_id, catalog.api_service
            )));
        }
        if catalog.poll_interval_secs == 0 {
            return Err(config_error(format!(
                "inference route {} console_catalog poll_interval must be positive",
                route.route_id
            )));
        }
    }
    for (alias, model) in &route.models {
        if !(1..=1024).contains(&model.prefix_parallelism) {
            return Err(config_error("prefix_parallelism must be in 1..=1024"));
        }
        if crate::inference::RESERVED_PLATFORM_ALIASES.contains(&alias.as_str()) {
            return Err(config_error(format!(
                "inference route {} model alias '{alias}' is reserved by a platform model",
                route.route_id
            )));
        }
        if !valid_model_alias(alias) {
            return Err(config_error(format!(
                "inference route {} contains invalid model alias '{}'",
                route.route_id, alias
            )));
        }
        validate_model(route, alias, model, gateway, workers, scheduled_workers)?;
    }

    if route.listings.len() > MAX_MODELS_PER_ROUTE {
        return Err(config_error(format!(
            "inference route {} exceeds the {MAX_MODELS_PER_ROUTE} listing limit",
            route.route_id
        )));
    }
    let mut listing_ids = HashSet::new();
    for (sku, listing) in &route.listings {
        if !valid_model_alias(sku) {
            return Err(config_error(format!(
                "inference route {} contains invalid listing sku '{sku}'",
                route.route_id
            )));
        }
        if listing.listing_id.is_nil() || !listing_ids.insert(listing.listing_id) {
            return Err(config_error(format!(
                "inference route {} listing '{sku}' has a nil or duplicate listing_id",
                route.route_id
            )));
        }
        if !gateway.services.contains_key(&listing.service) {
            return Err(config_error(format!(
                "inference route {} listing '{sku}' references unknown service '{}'",
                route.route_id, listing.service
            )));
        }
    }

    for (credential_id, grant) in &route.grants {
        validate_grant(route, *credential_id, grant, credentials)?;
    }
    if let Some(principal) = &route.principal {
        validate_principal(route, principal)?;
    }
    let mut supplier_ids = route
        .models
        .values()
        .flat_map(|model| model.targets.iter().map(|target| target.target_id))
        .collect::<HashSet<_>>();
    if let Some(supplier) = &route.ocr_supplier {
        validate_platform_supplier(
            route,
            "ocr_supplier",
            InferenceProtocol::Ocr,
            supplier,
            gateway,
            &mut supplier_ids,
        )?;
    }
    if let Some(supplier) = &route.decision_supplier {
        validate_platform_supplier(
            route,
            "decision_supplier",
            InferenceProtocol::SystemOne,
            supplier,
            gateway,
            &mut supplier_ids,
        )?;
    }
    if let Some(supplier) = &route.embedding_supplier {
        validate_platform_supplier(
            route,
            "embedding_supplier",
            InferenceProtocol::Openai,
            supplier,
            gateway,
            &mut supplier_ids,
        )?;
    }
    if let Some(supplier) = &route.rerank_supplier {
        validate_platform_supplier(
            route,
            "rerank_supplier",
            InferenceProtocol::Openai,
            supplier,
            gateway,
            &mut supplier_ids,
        )?;
    }

    Ok(())
}

fn validate_platform_supplier(
    route: &InferenceRouteConfig,
    slot: &str,
    protocol: InferenceProtocol,
    supplier: &InferenceTargetConfig,
    gateway: &GatewayConfig,
    seen_ids: &mut HashSet<Uuid>,
) -> Result<()> {
    if supplier.target_id.is_nil() || !seen_ids.insert(supplier.target_id) {
        return Err(config_error(format!(
            "inference route {} {slot} target {} must be a non-nil id that does not repeat a model or supplier target",
            route.route_id, supplier.target_id
        )));
    }
    if supplier.protocol != protocol {
        return Err(config_error(format!(
            "inference route {} {slot} protocol must be {}",
            route.route_id,
            protocol_name(protocol)
        )));
    }
    if !gateway.services.contains_key(&supplier.service) {
        return Err(config_error(format!(
            "inference route {} {slot} references unknown service '{}'",
            route.route_id, supplier.service
        )));
    }
    if !valid_model_alias(&supplier.upstream_model) {
        return Err(config_error(format!(
            "inference route {} {slot} has an invalid upstream_model",
            route.route_id
        )));
    }
    if supplier.weight == 0 {
        return Err(config_error(format!(
            "inference route {} {slot} weight must be positive",
            route.route_id
        )));
    }
    if let Some(name) = &supplier.api_key_env {
        if !valid_env_name(name) {
            return Err(config_error(format!(
                "inference route {} {slot} api_key_env must be an environment variable name",
                route.route_id
            )));
        }
    }
    Ok(())
}

fn protocol_name(protocol: InferenceProtocol) -> &'static str {
    match protocol {
        InferenceProtocol::Openai => "openai",
        InferenceProtocol::Anthropic => "anthropic",
        InferenceProtocol::Ocr => "ocr",
        InferenceProtocol::SystemOne => "systemone",
    }
}

const MAX_PRINCIPAL_CACHE_TTL_SECS: u64 = 3_600;

fn validate_principal(
    route: &InferenceRouteConfig,
    principal: &InferencePrincipalConfig,
) -> Result<()> {
    let route_id = route.route_id;
    let url = url::Url::parse(&principal.verify_url).map_err(|error| {
        config_error(format!(
            "inference route {route_id} principal verify_url is invalid: {error}"
        ))
    })?;
    if !matches!(url.scheme(), "http" | "https") || url.host_str().is_none() {
        return Err(config_error(format!(
            "inference route {route_id} principal verify_url must be an absolute http(s) URL"
        )));
    }
    for header in std::iter::once(&principal.subject_header).chain(principal.org_header.iter()) {
        if http::HeaderName::from_bytes(header.as_bytes()).is_err() {
            return Err(config_error(format!(
                "inference route {route_id} principal header '{header}' is not a valid header name"
            )));
        }
    }
    if principal.cache_ttl_secs == 0 || principal.cache_ttl_secs > MAX_PRINCIPAL_CACHE_TTL_SECS {
        return Err(config_error(format!(
            "inference route {route_id} principal cache_ttl_secs must be 1 to {MAX_PRINCIPAL_CACHE_TTL_SECS}"
        )));
    }
    let mut models = HashSet::new();
    for alias in &principal.models {
        if !models.insert(alias) || (alias != "*" && !route.models.contains_key(alias)) {
            return Err(config_error(format!(
                "inference route {route_id} principal contains an unknown or duplicate model alias '{alias}'"
            )));
        }
    }
    let endpoints = principal.endpoints.iter().copied().collect::<HashSet<_>>();
    if endpoints.is_empty()
        || endpoints.len() != principal.endpoints.len()
        || endpoints.contains(&InferenceEndpoint::Tools)
    {
        return Err(config_error(format!(
            "inference route {route_id} principal must contain unique non-tool endpoints"
        )));
    }
    validate_limits(route_id, Uuid::nil(), &principal.limits)
}

fn valid_env_name(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 255
        && !name.starts_with(|c: char| c.is_ascii_digit())
        && name
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'_')
}

fn validate_model(
    route: &InferenceRouteConfig,
    alias: &str,
    model: &InferenceModelConfig,
    gateway: &GatewayConfig,
    workers: &HashMap<String, InferenceWorkerConfig>,
    scheduled_workers: &mut HashMap<String, Uuid>,
) -> Result<()> {
    if model.model_id.is_nil() {
        return Err(config_error(format!(
            "inference model alias '{alias}' on route {} has a nil model_id",
            route.route_id
        )));
    }
    if model.targets.is_empty() || model.targets.len() > MAX_TARGETS_PER_MODEL {
        return Err(config_error(format!(
            "inference model alias '{alias}' on route {} must contain 1 to {MAX_TARGETS_PER_MODEL} targets",
            route.route_id
        )));
    }
    if let Some(scheduling) = &model.scheduling {
        validate_scheduling(route, alias, scheduling)?;
    }

    // Uniqueness is per model: multiple aliases may share one Power target
    // (for example chat + embeddings on the same worker).
    let mut model_target_ids = HashSet::new();
    let mut priorities = BTreeSet::new();
    let mut weight_by_priority = HashMap::<u32, u64>::new();
    let mut previous_priority = None;
    for target in &model.targets {
        if target.target_id.is_nil() || !model_target_ids.insert(target.target_id) {
            return Err(config_error(format!(
                "inference target IDs must be non-nil and unique; invalid target {}",
                target.target_id
            )));
        }
        if !gateway.services.contains_key(&target.service) {
            return Err(config_error(format!(
                "inference target {} references unknown service '{}'",
                target.target_id, target.service
            )));
        }
        if !valid_model_alias(&target.upstream_model) {
            return Err(config_error(format!(
                "inference target {} has an invalid upstream_model",
                target.target_id
            )));
        }
        if target.weight == 0 {
            return Err(config_error(format!(
                "inference target {} weight must be positive",
                target.target_id
            )));
        }
        if let Some(name) = &target.api_key_env {
            if !valid_env_name(name) {
                return Err(config_error(format!(
                    "inference target {} api_key_env must be an environment variable name",
                    target.target_id
                )));
            }
            if model.scheduling.is_some() {
                return Err(config_error(format!(
                    "inference target {} cannot combine api_key_env with worker scheduling",
                    target.target_id
                )));
            }
        }
        if previous_priority.is_some_and(|previous| target.priority < previous) {
            return Err(config_error(format!(
                "inference model alias '{alias}' targets must be ordered by ascending priority"
            )));
        }
        previous_priority = Some(target.priority);
        priorities.insert(target.priority);
        let total = weight_by_priority.entry(target.priority).or_default();
        *total = total.checked_add(u64::from(target.weight)).ok_or_else(|| {
            config_error(format!(
                "inference target priority {} weight overflows",
                target.priority
            ))
        })?;
        if *total > u64::from(u32::MAX) {
            return Err(config_error(format!(
                "inference target priority {} total weight exceeds u32",
                target.priority
            )));
        }
        if let Some(scheduling) = &model.scheduling {
            validate_scheduled_target(
                alias,
                target,
                scheduling,
                gateway,
                workers,
                scheduled_workers,
            )?;
        }
    }

    if priorities
        .iter()
        .copied()
        .enumerate()
        .any(|(expected, actual)| usize::try_from(actual) != Ok(expected))
    {
        return Err(config_error(format!(
            "inference model alias '{alias}' target priorities must be contiguous from zero"
        )));
    }

    Ok(())
}

fn validate_grant(
    route: &InferenceRouteConfig,
    credential_id: Uuid,
    grant: &InferenceGrantConfig,
    credentials: &HashMap<Uuid, InferenceCredentialConfig>,
) -> Result<()> {
    let credential = credentials.get(&credential_id).ok_or_else(|| {
        config_error(format!(
            "inference route {} grant references unknown credential {credential_id}",
            route.route_id
        ))
    })?;
    if credential.environment_id != route.environment_id {
        return Err(config_error(format!(
            "inference route {} grant credential {credential_id} belongs to another environment",
            route.route_id
        )));
    }
    if credential.revoked {
        return Err(config_error(format!(
            "inference route {} grants revoked credential {credential_id}",
            route.route_id
        )));
    }
    if grant.credential_generation == 0 || grant.credential_generation != credential.generation {
        return Err(config_error(format!(
            "inference route {} grant does not match credential {credential_id} generation",
            route.route_id
        )));
    }
    if grant.models.is_empty() {
        return Err(config_error(format!(
            "inference route {} grant for credential {credential_id} has no models",
            route.route_id
        )));
    }
    let mut models = HashSet::new();
    for alias in &grant.models {
        if !models.insert(alias) || !route.models.contains_key(alias) {
            return Err(config_error(format!(
                "inference route {} grant for credential {credential_id} contains an unknown or duplicate model alias '{alias}'",
                route.route_id
            )));
        }
    }
    let mut listings = HashSet::new();
    for sku in &grant.listings {
        if !listings.insert(sku) || !route.listings.contains_key(sku) {
            return Err(config_error(format!(
                "inference route {} grant for credential {credential_id} contains an unknown or duplicate listing sku '{sku}'",
                route.route_id
            )));
        }
    }
    if !grant.listings.is_empty()
        && !grant
            .endpoints
            .contains(&crate::config::InferenceEndpoint::Tools)
    {
        return Err(config_error(format!(
            "inference route {} grant for credential {credential_id} exposes listings without the tools endpoint",
            route.route_id
        )));
    }
    if grant.endpoints.is_empty()
        || grant
            .endpoints
            .iter()
            .copied()
            .collect::<HashSet<_>>()
            .len()
            != grant.endpoints.len()
    {
        return Err(config_error(format!(
            "inference route {} grant for credential {credential_id} must contain unique endpoints",
            route.route_id
        )));
    }
    if grant
        .endpoints
        .contains(&crate::config::InferenceEndpoint::Embeddings)
        && grant.models.iter().any(|alias| {
            route.models.get(alias).is_some_and(|model| {
                model
                    .scheduling
                    .as_ref()
                    .is_some_and(|scheduling| scheduling.distributed_serving.is_some())
            })
        })
    {
        return Err(config_error(format!(
            "inference route {} grant for credential {credential_id} exposes embeddings for a distributed prefill/decode model",
            route.route_id
        )));
    }
    validate_limits(route.route_id, credential_id, &grant.limits)
}

fn validate_limits(
    route_id: Uuid,
    credential_id: Uuid,
    limits: &InferenceLimitsConfig,
) -> Result<()> {
    if limits.max_concurrent_requests == 0
        || limits.requests_per_minute == 0
        || limits.request_burst == 0
        || limits.tokens_per_minute == 0
        || limits.request_burst > limits.requests_per_minute
    {
        return Err(config_error(format!(
            "inference route {route_id} grant for credential {credential_id} has invalid limits"
        )));
    }
    Ok(())
}

fn validate_credential_prefix(prefix: &str) -> Result<()> {
    let Some(suffix) = prefix.strip_prefix("a3s_inf_") else {
        return Err(config_error(
            "inference credential prefix must start with 'a3s_inf_'",
        ));
    };
    if suffix.len() < 8
        || suffix.len() > 32
        || !suffix
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit())
    {
        return Err(config_error(
            "inference credential prefix suffix must contain 8 to 32 lowercase ASCII letters or digits",
        ));
    }
    Ok(())
}

fn validate_argon2id_verifier(verifier_hash: &str) -> Result<()> {
    if verifier_hash.is_empty() || verifier_hash.len() > MAX_VERIFIER_BYTES {
        return Err(config_error(format!(
            "inference credential verifier_hash must contain 1 to {MAX_VERIFIER_BYTES} bytes"
        )));
    }
    PasswordHash::new(verifier_hash).map_err(|_| {
        config_error("inference credential verifier_hash is not a valid PHC string")
    })?;

    let parts = verifier_hash.split('$').collect::<Vec<_>>();
    if parts.len() != 6 || parts[1] != "argon2id" || parts[2] != "v=19" {
        return Err(config_error(
            "inference credential verifier_hash must use Argon2id PHC version 19",
        ));
    }
    let mut memory = None;
    let mut iterations = None;
    let mut lanes = None;
    for parameter in parts[3].split(',') {
        let (name, value) = parameter.split_once('=').ok_or_else(|| {
            config_error("inference credential verifier_hash has invalid Argon2id parameters")
        })?;
        let value = value.parse::<u32>().map_err(|_| {
            config_error("inference credential verifier_hash has invalid Argon2id parameters")
        })?;
        match name {
            "m" if memory.replace(value).is_none() => {}
            "t" if iterations.replace(value).is_none() => {}
            "p" if lanes.replace(value).is_none() => {}
            _ => {
                return Err(config_error(
                    "inference credential verifier_hash must contain exactly m, t, and p parameters",
                ));
            }
        }
    }
    let (Some(memory), Some(iterations), Some(lanes)) = (memory, iterations, lanes) else {
        return Err(config_error(
            "inference credential verifier_hash must contain m, t, and p parameters",
        ));
    };
    if !(MIN_ARGON2_MEMORY_KIB..=MAX_ARGON2_MEMORY_KIB).contains(&memory)
        || !(MIN_ARGON2_ITERATIONS..=MAX_ARGON2_ITERATIONS).contains(&iterations)
        || !(1..=MAX_ARGON2_LANES).contains(&lanes)
        || !(MIN_ARGON2_SALT_ENCODED_LEN..=MAX_ARGON2_SALT_ENCODED_LEN).contains(&parts[4].len())
        || !(MIN_ARGON2_OUTPUT_ENCODED_LEN..=MAX_ARGON2_OUTPUT_ENCODED_LEN)
            .contains(&parts[5].len())
    {
        return Err(config_error(
            "inference credential verifier_hash uses unsupported Argon2id cost, salt, or output bounds",
        ));
    }
    Ok(())
}

fn config_error(message: impl Into<String>) -> GatewayError {
    GatewayError::Config(message.into())
}
