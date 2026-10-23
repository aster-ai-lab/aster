//! Native inference request handling.

mod access_error;
mod authorization;
mod console_catalog;
mod decision;
mod decision_cache;
mod decision_plane;
mod distributed_serving;
mod fast_model;
mod identity;
mod limits;
mod native_wire;
mod observed_tokens;
mod openai_request;
mod platform_models;
mod prefix_affinity;
mod prefix_feedback;
mod principal;
mod routing;
mod scheduling;
mod service_meter;
mod standards;
mod supplier_keys;
mod token_reconcile;
mod tokenizer;
mod usage_report;
mod work_envelope;

pub(crate) use access_error::InferenceAccessError;
pub(crate) use principal::shared_token_admits;
pub(crate) use authorization::{
    AuthenticatedInference, InferenceAdmissionGuard, InferenceAuthorizer, InferenceDispatchTarget,
};
pub(crate) use console_catalog::maybe_spawn_catalog_poller;
pub(crate) use decision::{
    DecisionAssessment, DecisionContentPart, DecisionDefinition, DecisionError, DecisionRequest,
    DecisionResponse, DecisionResult, DecisionService, DecisionStepPart, DecisionVisibleTurn,
    DECISION_REQUEST_LIMIT_BYTES,
};
pub(crate) use decision_cache::{
    ChosenGroup, DecisionReason, FrozenDecision, TurnDecisionCache, TurnIdentity,
};
pub(crate) use decision_plane::{DecisionPlane, HttpDecisionTransport};
pub(crate) use distributed_serving::{
    DistributedExecutionRequest, DistributedInferenceResponse, DistributedServingError,
    DistributedServingOrchestrator, DistributedWorkerEndpoint, PowerDistributedClient,
    ProtocolBinding,
};
pub(crate) use identity::{InferenceAttemptIdentity, InferenceRequestIdentity};
#[cfg(test)]
pub(crate) use identity::{ATTEMPT_ID_HEADER, REQUEST_ID_HEADER};
pub(crate) use limits::TokenReservation;
pub(crate) use native_wire::{
    mark_native_wire, native_models_body, render_native_error, with_native_wire_scope,
    EnvelopeError, NativeWire,
};
pub(crate) use observed_tokens::{LifecycleTokenBreakdown, ObservedTokenUsage};
pub(crate) use platform_models::{
    serve_platform_alias_model, serve_platform_model, warm_platform_engines, ServedPlatformModel,
};
pub(crate) use prefix_affinity::{target_signature, PrefixHint, PrefixLease, PrefixTarget};
pub(crate) use prefix_feedback::track_prefix_body;
pub(crate) use principal::PrincipalSubject;
pub(crate) use routing::{
    build_decision_request, decision_inputs, effective_targets, evaluate_routing,
    plan_decision_definition, turn_identity_from_document, DecisionInputs, RouteDecision,
};
/// Model aliases the in-process platform engines answer to. A route may
/// never declare these: the platform models own them.
pub(crate) const PLATFORM_OCR_ALIAS: &str = "platform/pp-ocr-v6";
pub(crate) const PLATFORM_APOFASI_ALIAS: &str = "platform/apofasi";
pub(crate) const PLATFORM_EMBEDDING_ALIAS: &str = "platform/bge-m3";
pub(crate) const PLATFORM_RERANK_ALIAS: &str = "platform/bge-reranker-v2-m3";
pub(crate) const RESERVED_PLATFORM_ALIASES: [&str; 2] =
    [PLATFORM_EMBEDDING_ALIAS, PLATFORM_RERANK_ALIAS];

pub(crate) use openai_request::{
    collect_json_body, collect_proxy_json_body, models_response, tool_invoke_sku,
    valid_model_alias, OpenAiJsonRequest, OpenAiRequestError, OpenAiRequestProfile,
    OPENAI_REQUEST_BODY_LIMIT,
};
pub(crate) use scheduling::{
    InferenceWorkerCandidate, InferenceWorkerPairSelection, InferenceWorkerPairSelectionRequest,
    InferenceWorkerSelection, InferenceWorkerSelectionRequest,
};
pub(crate) use supplier_keys::load_supplier_credentials;
pub(crate) use token_reconcile::{
    track_attributed_token_budget_response, track_token_budget_response, UsageAttribution,
};
pub(crate) use work_envelope::wrap_json_success_body;

#[cfg(test)]
mod cloud_worker_acl_render_contract_tests;
#[cfg(test)]
mod power_observation_contract_tests;
#[cfg(test)]
mod power_observation_projection_tests;
#[cfg(test)]
mod service_meter_tests;
#[cfg(test)]
mod standards_tests;
#[cfg(test)]
mod tokenizer_revision_contract_tests;
