//! Closed OpenAI endpoint matching and bounded JSON request collection.

use super::standards::{ModelStandard, StandardAdmission};

#[path = "openai_message_annotations.rs"]
mod message_annotations;
use bytes::Bytes;
use http::header::{CONTENT_LENGTH, CONTENT_TYPE};
use http::{HeaderMap, Method, Response, StatusCode};
use http_body_util::{BodyExt, LengthLimitError, Limited};
use serde::de::{Deserialize, Deserializer, IgnoredAny, MapAccess, Visitor};
use sha2::{Digest, Sha256};
use std::error::Error;
use std::fmt;

/// Fixed upper bound for a native OpenAI request body.
pub(crate) const OPENAI_REQUEST_BODY_LIMIT: usize = 8 * 1024 * 1024;

/// Fixed upper bound for an external OpenAI model alias.
const OPENAI_MODEL_ALIAS_LIMIT: usize = 255;
const PROMPT_CACHE_KEY_LIMIT: usize = 256;

/// A supported OpenAI-compatible request shape.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum OpenAiRequestProfile {
    /// List the models visible to the caller.
    Models,
    /// Create a chat completion.
    ChatCompletions,
    /// Create a legacy text completion.
    Completions,
    /// Create embeddings.
    Embeddings,
    /// Create an Anthropic message.
    Messages,
    /// Count Anthropic message input tokens.
    CountTokens,
    /// Recognize text in one image. The caller sends no model alias.
    Ocr,
    /// Answer a System One decision request. The caller sends no model alias.
    SystemOne,
    /// Rerank documents against one query (Jina/Cohere wire shape).
    Rerank,
    /// Create a response (OpenAI Responses API). Served natively
    /// (passthrough) or statelessly onto chat-completions when the target
    /// opted into translation.
    Responses,
}

impl OpenAiRequestProfile {
    /// Match admitted and platform catalog routes. Reserved catalog routes
    /// return `None` here and are refused by [`Self::is_reserved_route`]
    /// before ordinary routing. Query parameters do not affect the path match.
    pub(crate) fn match_request(method: &Method, path: &str) -> Option<Self> {
        let standard = ModelStandard::match_route(method.as_str(), path)?;
        match standard.admission() {
            StandardAdmission::Admitted | StandardAdmission::Platform => {
                Self::from_standard(standard)
            }
            StandardAdmission::Reserved => None,
        }
    }

    /// A catalog path whose gate has not admitted dispatch.
    pub(crate) fn is_reserved_route(method: &Method, path: &str) -> bool {
        ModelStandard::is_reserved_route(method.as_str(), path)
    }

    fn from_standard(standard: ModelStandard) -> Option<Self> {
        Some(match standard {
            ModelStandard::OpenaiModels => Self::Models,
            ModelStandard::OpenaiChat => Self::ChatCompletions,
            ModelStandard::OpenaiCompletions => Self::Completions,
            ModelStandard::OpenaiEmbeddings => Self::Embeddings,
            ModelStandard::Rerank => Self::Rerank,
            ModelStandard::Ocr => Self::Ocr,
            ModelStandard::DecisionSystemOne => Self::SystemOne,
            ModelStandard::OpenaiResponses
            | ModelStandard::AnthropicMessages
            | ModelStandard::GeminiGenerate
            | ModelStandard::GeminiEmbed
            | ModelStandard::OpenaiImages
            | ModelStandard::OpenaiAudio
            | ModelStandard::Api
            | ModelStandard::Mcp => return None,
        })
    }

    /// OCR and System One are served by a platform model or a bound supplier.
    /// They never enter chat or embeddings dispatch.
    pub(crate) const fn is_platform_model(self) -> bool {
        matches!(self, Self::Ocr | Self::SystemOne)
    }

    /// Whether this profile requires a bounded JSON request body.
    ///
    /// OCR carries image bytes. A missing model alias must not send those
    /// bytes down the JSON chat path.
    pub(crate) const fn requires_json_body(self) -> bool {
        !matches!(self, Self::Models | Self::Ocr)
    }

    /// Whether this endpoint can request an SSE response through its JSON
    /// `stream` field.
    pub(crate) const fn supports_streaming(self) -> bool {
        matches!(
            self,
            Self::ChatCompletions | Self::Completions | Self::Messages
        )
    }

    /// Whether errors use the Anthropic `{type:"error", error:{...}}` shape.
    pub(crate) const fn is_anthropic(self) -> bool {
        matches!(self, Self::Messages | Self::CountTokens)
    }
}

/// A stable OpenAI-compatible request rejection.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum OpenAiRequestError {
    /// The request does not declare an `application/json` media type.
    UnsupportedMediaType,
    /// The declared or observed request body exceeds the fixed limit.
    BodyTooLarge,
    /// The request body could not be read from the downstream connection.
    BodyReadFailed,
    /// The request body is not syntactically valid JSON.
    InvalidJson,
    /// The JSON request body is not an object.
    InvalidBodyShape,
    /// The JSON request object does not contain a model.
    MissingModel,
    /// The model is not a bounded, non-empty string.
    InvalidModel,
    /// The optional prompt-cache affinity key is not a bounded identifier.
    InvalidPromptCacheKey,
}

impl OpenAiRequestError {
    /// HTTP status associated with this stable rejection.
    pub(crate) const fn status(self) -> StatusCode {
        match self {
            Self::UnsupportedMediaType => StatusCode::UNSUPPORTED_MEDIA_TYPE,
            Self::BodyTooLarge => StatusCode::PAYLOAD_TOO_LARGE,
            Self::BodyReadFailed
            | Self::InvalidJson
            | Self::InvalidBodyShape
            | Self::MissingModel
            | Self::InvalidModel
            | Self::InvalidPromptCacheKey => StatusCode::BAD_REQUEST,
        }
    }

    fn message(self) -> &'static str {
        match self {
            Self::UnsupportedMediaType => "Content-Type must be application/json.",
            Self::BodyTooLarge => "Request body exceeds the 8 MiB limit.",
            Self::BodyReadFailed => "Request body could not be read.",
            Self::InvalidJson => "Request body must contain valid JSON.",
            Self::InvalidBodyShape => "Request body must be a JSON object.",
            Self::MissingModel => "Required field 'model' is missing.",
            Self::InvalidModel => {
                "Field 'model' must be a non-empty string of at most 255 bytes."
            }
            Self::InvalidPromptCacheKey => {
                "Field 'prompt_cache_key' must be a non-empty string of at most 256 bytes without surrounding whitespace or control characters."
            }
        }
    }

    /// Convert the rejection into a Work Connect ApiEnvelope response.
    pub(crate) fn into_response(self) -> Response<Bytes> {
        self.into_response_with_request_id(uuid::Uuid::new_v4())
    }

    pub(crate) fn into_response_with_request_id(self, request_id: uuid::Uuid) -> Response<Bytes> {
        super::work_envelope::error_envelope_response(
            request_id,
            self.status(),
            self.message(),
            "request.invalid",
        )
    }
}

/// One validated OpenAI JSON request.
///
/// The parsed document remains available at the inference-dispatch boundary so
/// later authorization and model-routing stages do not need to parse it again.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct OpenAiJsonRequest {
    pub(crate) body: Bytes,
    pub(crate) document: serde_json::Map<String, serde_json::Value>,
}

/// A validated OpenAI request that will be forwarded without managed model
/// rewriting. Only the fields needed by the proxy boundary are retained.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct OpenAiProxyRequest {
    body: Bytes,
}

impl OpenAiProxyRequest {
    /// Recover validated request bytes after shared message normalization.
    pub(crate) fn into_body(self) -> Bytes {
        self.body
    }
}

impl OpenAiJsonRequest {
    /// Validated external model alias selected by the client.
    /// Original request bytes for platform-model wire parsing.
    pub(crate) fn body_bytes(&self) -> &Bytes {
        &self.body
    }

    pub(crate) fn model_alias(&self) -> &str {
        self.document
            .get("model")
            .and_then(serde_json::Value::as_str)
            .unwrap_or_default()
    }

    /// Whether the validated request explicitly selects OpenAI SSE streaming.
    pub(crate) fn stream_requested(&self) -> bool {
        self.document
            .get("stream")
            .and_then(serde_json::Value::as_bool)
            .unwrap_or(false)
    }

    /// Recover validated request bytes after shared message normalization.
    pub(crate) fn into_body(self) -> Bytes {
        self.body
    }

    /// Derive the only cache identifier allowed to cross the managed boundary.
    /// The client value is never returned or retained outside this request.
    pub(crate) fn scoped_prompt_cache_key(
        &self,
        credential_id: uuid::Uuid,
        model_id: uuid::Uuid,
        endpoint: crate::config::InferenceEndpoint,
    ) -> Option<String> {
        let caller_key = self
            .document
            .get("prompt_cache_key")
            .and_then(serde_json::Value::as_str)?;
        let mut digest = Sha256::new();
        hash_part(&mut digest, b"a3s.gateway.prompt-cache-key.v1");
        hash_part(&mut digest, credential_id.as_bytes());
        hash_part(&mut digest, model_id.as_bytes());
        hash_part(&mut digest, endpoint.as_str().as_bytes());
        hash_part(&mut digest, caller_key.as_bytes());
        Some(format!("a3s-gw-pcache-v1:{:x}", digest.finalize()))
    }

    /// Fail-closed local token-budget reservation until trusted usage lands.
    ///
    /// Uses [`super::tokenizer`] (`a3s.gateway.tokenizer.v1`) for input text
    /// plus the request's completion budget (`max_completion_tokens` or
    /// `max_tokens`). This is not a Cloud billing tokenizer and must be
    /// reconciled when upstream `usage` is available.
    pub(crate) fn estimate_token_reservation(&self, profile: OpenAiRequestProfile) -> u64 {
        self.estimate_token_reservation_split(profile)
            .total()
            .max(1)
    }

    /// [`Self::estimate_token_reservation`] split by price lane: the input
    /// estimate prices at the model's input rate, the completion budget at
    /// its output rate.
    pub(crate) fn estimate_token_reservation_split(
        &self,
        profile: OpenAiRequestProfile,
    ) -> super::limits::TokenReservation {
        let input = match profile {
            // The responses dialect carries instructions + input items, not
            // a messages array.
            OpenAiRequestProfile::Responses => approximate_responses_input_tokens(&self.document),
            _ => approximate_input_tokens(&self.document),
        };
        let completion = match profile {
            OpenAiRequestProfile::Embeddings
            | OpenAiRequestProfile::Models
            | OpenAiRequestProfile::CountTokens
            | OpenAiRequestProfile::Ocr
            | OpenAiRequestProfile::SystemOne
            | OpenAiRequestProfile::Rerank => 0,
            OpenAiRequestProfile::Responses => self
                .document
                .get("max_output_tokens")
                .and_then(serde_json::Value::as_u64)
                .unwrap_or(DEFAULT_COMPLETION_TOKEN_RESERVATION),
            OpenAiRequestProfile::ChatCompletions
            | OpenAiRequestProfile::Completions
            | OpenAiRequestProfile::Messages => completion_token_budget(&self.document),
        };
        super::limits::TokenReservation { input, completion }
    }

    /// Ask an OpenAI-compatible upstream to report usage on the final SSE
    /// chunk so streamed requests can be metered. A caller-provided
    /// `stream_options.include_usage` is left as is.
    pub(crate) fn request_stream_usage(&mut self) {
        if !self.stream_requested() {
            return;
        }
        let options = self
            .document
            .entry("stream_options".to_string())
            .or_insert_with(|| serde_json::Value::Object(serde_json::Map::new()));
        if let serde_json::Value::Object(options) = options {
            options
                .entry("include_usage".to_string())
                .or_insert(serde_json::Value::Bool(true));
        }
    }

    /// Build a replayable managed dispatch body for one upstream target.
    pub(crate) fn routed_body(
        &self,
        upstream_model: &str,
        scoped_prompt_cache_key: Option<&str>,
    ) -> Result<Bytes, serde_json::Error> {
        let mut document = self.document.clone();
        document.insert(
            "model".to_string(),
            serde_json::Value::String(upstream_model.to_string()),
        );
        if document.contains_key("prompt_cache_key") {
            match scoped_prompt_cache_key {
                Some(key) => {
                    document.insert(
                        "prompt_cache_key".to_string(),
                        serde_json::Value::String(key.to_string()),
                    );
                }
                None => {
                    document.remove("prompt_cache_key");
                }
            }
        }
        serde_json::to_vec(&document).map(Bytes::from)
    }
}

fn hash_part(digest: &mut Sha256, value: &[u8]) {
    digest.update((value.len() as u64).to_be_bytes());
    digest.update(value);
}

/// Default completion reservation when the client omits an explicit budget.
const DEFAULT_COMPLETION_TOKEN_RESERVATION: u64 = 1_024;

fn completion_token_budget(document: &serde_json::Map<String, serde_json::Value>) -> u64 {
    document
        .get("max_completion_tokens")
        .or_else(|| document.get("max_tokens"))
        .and_then(serde_json::Value::as_u64)
        .unwrap_or(DEFAULT_COMPLETION_TOKEN_RESERVATION)
}

/// Estimate the input lanes of a responses-dialect body: instructions
/// plus every text carried by the `input` field (string or item list).
fn approximate_responses_input_tokens(
    document: &serde_json::Map<String, serde_json::Value>,
) -> u64 {
    let mut total = 0_u64;
    if let Some(instructions) = document
        .get("instructions")
        .and_then(serde_json::Value::as_str)
    {
        total = total.saturating_add(super::tokenizer::estimate_text_tokens(instructions));
    }
    match document.get("input") {
        Some(serde_json::Value::String(text)) => {
            total = total.saturating_add(super::tokenizer::estimate_text_tokens(text));
        }
        Some(serde_json::Value::Array(items)) => {
            for item in items {
                match item.get("content") {
                    Some(serde_json::Value::String(text)) => {
                        total = total.saturating_add(super::tokenizer::estimate_text_tokens(text));
                    }
                    Some(serde_json::Value::Array(parts)) => {
                        for part in parts {
                            if let Some(text) = part.get("text").and_then(serde_json::Value::as_str)
                            {
                                total = total
                                    .saturating_add(super::tokenizer::estimate_text_tokens(text));
                            }
                        }
                    }
                    _ => {}
                }
                if let Some(role) = item.get("role").and_then(serde_json::Value::as_str) {
                    total = total.saturating_add(super::tokenizer::estimate_text_tokens(role));
                }
            }
        }
        _ => {}
    }
    total.max(1)
}

fn approximate_input_tokens(document: &serde_json::Map<String, serde_json::Value>) -> u64 {
    let mut total = 0_u64;
    if let Some(messages) = document
        .get("messages")
        .and_then(serde_json::Value::as_array)
    {
        for message in messages {
            total = total.saturating_add(json_text_tokens(message.get("content")));
            if let Some(role) = message.get("role").and_then(serde_json::Value::as_str) {
                total = total.saturating_add(super::tokenizer::estimate_text_tokens(role));
            }
        }
    }
    total = total.saturating_add(json_text_tokens(document.get("system")));
    total = total.saturating_add(json_text_tokens(document.get("prompt")));
    total = total.saturating_add(json_text_tokens(document.get("input")));
    total = total.saturating_add(json_text_tokens(document.get("query")));
    total = total.saturating_add(json_text_tokens(document.get("documents")));
    total
}

fn json_text_tokens(value: Option<&serde_json::Value>) -> u64 {
    match value {
        Some(serde_json::Value::String(text)) => super::tokenizer::estimate_text_tokens(text),
        Some(serde_json::Value::Array(parts)) => parts
            .iter()
            .map(|part| match part {
                serde_json::Value::String(text) => super::tokenizer::estimate_text_tokens(text),
                serde_json::Value::Object(object) => object
                    .get("text")
                    .and_then(serde_json::Value::as_str)
                    .map(super::tokenizer::estimate_text_tokens)
                    .unwrap_or(0),
                _ => 0,
            })
            .fold(0_u64, u64::saturating_add),
        _ => 0,
    }
}

/// Build a Work Connect ApiEnvelope wrapping an OpenAI-compatible model list.
pub(crate) fn models_response(
    models: &[String],
    request_id: uuid::Uuid,
) -> Result<Response<Bytes>, serde_json::Error> {
    let data = models
        .iter()
        .map(|model| {
            serde_json::json!({
                "id": model,
                "object": "model",
                "created": 0,
                "owned_by": "a3s"
            })
        })
        .collect::<Vec<_>>();
    let list = serde_json::json!({
        "object": "list",
        "data": data
    });
    let body = super::work_envelope::ok_envelope(list, request_id);
    Ok(super::work_envelope::envelope_response(
        StatusCode::OK,
        body,
        request_id,
    ))
}

#[derive(Debug, Clone, Copy)]
enum ProxyJsonField {
    Model,
    Other,
}

struct ProxyJsonFieldVisitor;

impl Visitor<'_> for ProxyJsonFieldVisitor {
    type Value = ProxyJsonField;

    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("an OpenAI request field")
    }

    fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
    where
        E: serde::de::Error,
    {
        Ok(match value {
            "model" => ProxyJsonField::Model,
            _ => ProxyJsonField::Other,
        })
    }

    fn visit_borrowed_str<E>(self, value: &'_ str) -> Result<Self::Value, E>
    where
        E: serde::de::Error,
    {
        self.visit_str(value)
    }

    fn visit_string<E>(self, value: String) -> Result<Self::Value, E>
    where
        E: serde::de::Error,
    {
        self.visit_str(&value)
    }
}

impl<'de> Deserialize<'de> for ProxyJsonField {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_identifier(ProxyJsonFieldVisitor)
    }
}

#[derive(Default)]
struct ProxyJsonMetadata {
    model: Option<serde_json::Value>,
}

struct ProxyJsonMetadataVisitor;

impl<'de> Visitor<'de> for ProxyJsonMetadataVisitor {
    type Value = ProxyJsonMetadata;

    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("an OpenAI JSON request object")
    }

    fn visit_map<M>(self, mut map: M) -> Result<Self::Value, M::Error>
    where
        M: MapAccess<'de>,
    {
        let mut metadata = ProxyJsonMetadata::default();
        while let Some(field) = map.next_key::<ProxyJsonField>()? {
            match field {
                ProxyJsonField::Model => metadata.model = Some(map.next_value()?),
                ProxyJsonField::Other => {
                    map.next_value::<IgnoredAny>()?;
                }
            }
        }
        Ok(metadata)
    }
}

impl<'de> Deserialize<'de> for ProxyJsonMetadata {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_map(ProxyJsonMetadataVisitor)
    }
}

/// Collect and validate an OpenAI JSON request that does not require managed
/// model rewriting. Unknown fields are syntactically validated and discarded
/// instead of being allocated into a complete JSON document.
pub(crate) async fn collect_proxy_json_body<B>(
    headers: &HeaderMap,
    body: B,
) -> Result<OpenAiProxyRequest, OpenAiRequestError>
where
    B: hyper::body::Body<Data = Bytes>,
    B::Error: Into<Box<dyn Error + Send + Sync>>,
{
    let body = collect_bounded_json_body(headers, body).await?;
    // Fast single-pass path through the fast_model module: one Value parse,
    // distinct error types preserved (InvalidJson vs InvalidBodyShape vs
    // MissingModel vs InvalidModel). This replaces the previous two-pass
    // approach (IgnoredAny validation + ProxyJsonMetadata visitor) and
    // halves the JSON parsing work per request on the hot proxy path.
    let mut value: serde_json::Value =
        serde_json::from_slice(&body).map_err(|_| OpenAiRequestError::InvalidJson)?;
    let document = value
        .as_object_mut()
        .ok_or(OpenAiRequestError::InvalidBodyShape)?;
    let normalized = message_annotations::normalize(document);
    let model = value
        .get("model")
        .ok_or(OpenAiRequestError::MissingModel)?
        .as_str()
        .ok_or(OpenAiRequestError::InvalidModel)?
        .to_owned();
    if !valid_model_alias(&model) {
        return Err(OpenAiRequestError::InvalidModel);
    }
    let body = if normalized {
        encode_normalized_body(&value)?
    } else {
        body
    };
    Ok(OpenAiProxyRequest { body })
}

fn validate_prompt_cache_key(value: Option<&serde_json::Value>) -> Result<(), OpenAiRequestError> {
    let Some(value) = value else {
        return Ok(());
    };
    let Some(key) = value.as_str() else {
        return Err(OpenAiRequestError::InvalidPromptCacheKey);
    };
    if key.is_empty()
        || key.len() > PROMPT_CACHE_KEY_LIMIT
        || key.trim() != key
        || key.chars().any(char::is_control)
    {
        return Err(OpenAiRequestError::InvalidPromptCacheKey);
    }
    Ok(())
}

/// Collect and validate one OpenAI JSON request body under the fixed limit.
///
/// Known client replay annotations are removed consistently for native and
/// proxy dispatch. Requests without annotations retain their original bytes.
/// The normalized bytes and document always describe the same message history.
pub(crate) async fn collect_json_body<B>(
    headers: &HeaderMap,
    body: B,
) -> Result<OpenAiJsonRequest, OpenAiRequestError>
where
    B: hyper::body::Body<Data = Bytes>,
    B::Error: Into<Box<dyn Error + Send + Sync>>,
{
    let body = collect_bounded_json_body(headers, body).await?;

    let mut document = match serde_json::from_slice::<serde_json::Value>(&body)
        .map_err(|_| OpenAiRequestError::InvalidJson)?
    {
        serde_json::Value::Object(document) => document,
        _ => return Err(OpenAiRequestError::InvalidBodyShape),
    };
    let model = document
        .get("model")
        .ok_or(OpenAiRequestError::MissingModel)?
        .as_str()
        .ok_or(OpenAiRequestError::InvalidModel)?;
    if !valid_model_alias(model) {
        return Err(OpenAiRequestError::InvalidModel);
    }
    validate_prompt_cache_key(document.get("prompt_cache_key"))?;
    let body = if message_annotations::normalize(&mut document) {
        encode_normalized_body(&document)?
    } else {
        body
    };

    Ok(OpenAiJsonRequest { body, document })
}

fn encode_normalized_body(value: &impl serde::Serialize) -> Result<Bytes, OpenAiRequestError> {
    let body = serde_json::to_vec(value).map_err(|_| OpenAiRequestError::InvalidJson)?;
    if body.len() > OPENAI_REQUEST_BODY_LIMIT {
        return Err(OpenAiRequestError::BodyTooLarge);
    }
    Ok(Bytes::from(body))
}

async fn collect_bounded_json_body<B>(
    headers: &HeaderMap,
    body: B,
) -> Result<Bytes, OpenAiRequestError>
where
    B: hyper::body::Body<Data = Bytes>,
    B::Error: Into<Box<dyn Error + Send + Sync>>,
{
    if !has_json_content_type(headers) {
        return Err(OpenAiRequestError::UnsupportedMediaType);
    }

    if declared_body_is_too_large(headers) {
        return Err(OpenAiRequestError::BodyTooLarge);
    }

    Limited::new(body, OPENAI_REQUEST_BODY_LIMIT)
        .collect()
        .await
        .map_err(|error| {
            if error.downcast_ref::<LengthLimitError>().is_some() {
                OpenAiRequestError::BodyTooLarge
            } else {
                OpenAiRequestError::BodyReadFailed
            }
        })
        .map(|collected| collected.to_bytes())
}

fn has_json_content_type(headers: &HeaderMap) -> bool {
    headers
        .get(CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.split(';').next())
        .is_some_and(|media_type| media_type.trim().eq_ignore_ascii_case("application/json"))
}

fn declared_body_is_too_large(headers: &HeaderMap) -> bool {
    headers
        .get(CONTENT_LENGTH)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.parse::<u64>().ok())
        .is_some_and(|length| length > OPENAI_REQUEST_BODY_LIMIT as u64)
}

/// Exact Work Connect tool path: `POST /v1/tools/{sku}/invoke`.
///
/// A trailing slash or extra segment is not a tool invoke. Callers must keep
/// that near-miss as an unsupported endpoint rather than an entitlement denial.
pub(crate) fn tool_invoke_sku<'a>(method: &Method, path: &'a str) -> Option<&'a str> {
    if *method != Method::POST {
        return None;
    }
    let rest = path.strip_prefix("/v1/tools/")?;
    let (sku, action) = rest.split_once('/')?;
    if action == "invoke" && valid_model_alias(sku) {
        Some(sku)
    } else {
        None
    }
}

pub(crate) fn valid_model_alias(model: &str) -> bool {
    !model.is_empty()
        && model.len() <= OPENAI_MODEL_ALIAS_LIMIT
        && model.trim() == model
        && !model.chars().any(char::is_control)
}

#[cfg(test)]
mod tests {
    use super::*;
    use http_body_util::Full;

    fn json_headers() -> HeaderMap {
        let mut headers = HeaderMap::new();
        headers.insert(CONTENT_TYPE, "application/json".parse().unwrap());
        headers
    }

    #[test]
    fn matches_only_the_closed_endpoint_and_method_set() {
        let supported = [
            (Method::GET, "/v1/models", OpenAiRequestProfile::Models),
            (
                Method::POST,
                "/v1/chat/completions",
                OpenAiRequestProfile::ChatCompletions,
            ),
            (
                Method::POST,
                "/v1/completions",
                OpenAiRequestProfile::Completions,
            ),
            (
                Method::POST,
                "/v1/embeddings",
                OpenAiRequestProfile::Embeddings,
            ),
            (Method::POST, "/v1/ocr", OpenAiRequestProfile::Ocr),
            (
                Method::POST,
                "/v1/systemone",
                OpenAiRequestProfile::SystemOne,
            ),
        ];

        for (method, path, expected) in supported {
            assert_eq!(
                OpenAiRequestProfile::match_request(&method, path),
                Some(expected)
            );
        }

        for (method, path) in [
            (Method::POST, "/v1/models"),
            (Method::GET, "/v1/chat/completions"),
            (Method::POST, "/v1/chat/completions/"),
            (Method::POST, "/prefix/v1/chat/completions"),
            (Method::POST, "/v1/responses/"),
            (Method::GET, "/v1/responses"),
            (Method::POST, "/benchmark/sse"),
            (Method::GET, "/v1/ocr"),
            (Method::GET, "/v1/systemone"),
            (Method::POST, "/v1/ocr/extra"),
            (Method::POST, "/v1/systemone/extra"),
            (Method::POST, "/v1/responses"),
            (Method::POST, "/v1/messages"),
            (Method::POST, "/v1/messages/count_tokens"),
            (Method::POST, "/v1/mcp"),
            (Method::POST, "/v1/service"),
        ] {
            assert_eq!(OpenAiRequestProfile::match_request(&method, path), None);
        }
        assert_eq!(
            OpenAiRequestProfile::match_request(&Method::POST, "/v1/rerank"),
            Some(OpenAiRequestProfile::Rerank)
        );
        assert!(OpenAiRequestProfile::is_reserved_route(
            &Method::POST,
            "/v1/responses"
        ));
        assert!(!OpenAiRequestProfile::is_reserved_route(
            &Method::POST,
            "/v1/chat/completions"
        ));
        assert!(!OpenAiRequestProfile::is_reserved_route(
            &Method::POST,
            "/v1/ocr"
        ));
    }

    #[test]
    fn only_post_profiles_require_json_bodies() {
        assert!(!OpenAiRequestProfile::Models.requires_json_body());
        assert!(OpenAiRequestProfile::ChatCompletions.requires_json_body());
        assert!(OpenAiRequestProfile::Completions.requires_json_body());
        assert!(OpenAiRequestProfile::Embeddings.requires_json_body());
        assert!(!OpenAiRequestProfile::Ocr.requires_json_body());
        assert!(OpenAiRequestProfile::SystemOne.requires_json_body());
        assert!(!OpenAiRequestProfile::Ocr.supports_streaming());
        assert!(!OpenAiRequestProfile::SystemOne.supports_streaming());
        assert!(OpenAiRequestProfile::Ocr.is_platform_model());
        assert!(OpenAiRequestProfile::SystemOne.is_platform_model());
        assert!(!OpenAiRequestProfile::ChatCompletions.is_platform_model());
        assert!(!OpenAiRequestProfile::Embeddings.is_platform_model());
    }

    #[test]
    fn only_completion_profiles_support_json_selected_streaming() {
        assert!(!OpenAiRequestProfile::Models.supports_streaming());
        assert!(OpenAiRequestProfile::ChatCompletions.supports_streaming());
        assert!(OpenAiRequestProfile::Completions.supports_streaming());
        assert!(!OpenAiRequestProfile::Embeddings.supports_streaming());
    }

    #[tokio::test]
    async fn estimates_token_reservation_from_input_and_completion_budget() {
        let chat = collect_json_body(
            &json_headers(),
            Full::new(Bytes::from_static(
                br#"{"model":"local","messages":[{"role":"user","content":"abcd"}],"max_tokens":30}"#,
            )),
        )
        .await
        .unwrap();
        // role "user" => 1; "abcd" => 1; completion budget 30.
        assert_eq!(
            chat.estimate_token_reservation(OpenAiRequestProfile::ChatCompletions),
            32
        );

        let embeddings = collect_json_body(
            &json_headers(),
            Full::new(Bytes::from_static(
                br#"{"model":"local","input":"abcdefgh","max_tokens":999}"#,
            )),
        )
        .await
        .unwrap();
        assert_eq!(
            embeddings.estimate_token_reservation(OpenAiRequestProfile::Embeddings),
            2
        );

        let cjk = collect_json_body(
            &json_headers(),
            Full::new(Bytes::from(
                r#"{"model":"local","messages":[{"role":"user","content":"你好"}],"max_tokens":1}"#
                    .as_bytes()
                    .to_vec(),
            )),
        )
        .await
        .unwrap();
        // role 1 + two CJK chars + completion 1
        assert_eq!(
            cjk.estimate_token_reservation(OpenAiRequestProfile::ChatCompletions),
            4
        );
    }

    #[tokio::test]
    async fn accepts_json_media_type_parameters_and_preserves_bytes() {
        let mut headers = HeaderMap::new();
        headers.insert(
            CONTENT_TYPE,
            "Application/JSON; charset=utf-8".parse().unwrap(),
        );
        let request = Bytes::from_static(br#"{ "model": "local" }"#);

        let collected = collect_json_body(&headers, Full::new(request.clone()))
            .await
            .unwrap();

        assert_eq!(collected.model_alias(), "local");
        assert_eq!(collected.into_body(), request);
    }

    #[tokio::test]
    async fn recognizes_only_a_boolean_true_stream_field() {
        for (body, expected) in [
            (br#"{"model":"local","stream":true}"#.as_slice(), true),
            (br#"{"model":"local","stream":false}"#.as_slice(), false),
            (br#"{"model":"local","stream":"true"}"#.as_slice(), false),
            (br#"{"model":"local"}"#.as_slice(), false),
        ] {
            let request =
                collect_json_body(&json_headers(), Full::new(Bytes::copy_from_slice(body)))
                    .await
                    .unwrap();
            assert_eq!(request.stream_requested(), expected);
        }
    }

    #[tokio::test]
    async fn proxy_validation_discards_unknown_fields_and_preserves_bytes() {
        let request = Bytes::from_static(
            br#"{"model":"local","messages":[{"role":"user","content":"hello"}],"metadata":{"tenant":"acme"},"stream":true}"#,
        );

        let collected = collect_proxy_json_body(&json_headers(), Full::new(request.clone()))
            .await
            .unwrap();

        assert_eq!(collected.into_body(), request);
    }

    #[tokio::test]
    async fn proxy_validation_keeps_last_duplicate_field_semantics() {
        let collected = collect_proxy_json_body(
            &json_headers(),
            Full::new(Bytes::from_static(
                br#"{"model":42,"model":"local","stream":true}"#,
            )),
        )
        .await
        .unwrap();

        assert_eq!(
            collected.into_body(),
            br#"{"model":42,"model":"local","stream":true}"#.as_slice()
        );
    }

    #[tokio::test]
    async fn proxy_validation_enforces_model_alias_bounds_without_rewriting() {
        let at_limit = "x".repeat(OPENAI_MODEL_ALIAS_LIMIT);
        let body = Bytes::from(format!(r#"{{"model":"{at_limit}","messages":[]}}"#));
        let collected = collect_proxy_json_body(&json_headers(), Full::new(body.clone()))
            .await
            .unwrap();
        assert_eq!(collected.into_body(), body);

        let over_limit = "x".repeat(OPENAI_MODEL_ALIAS_LIMIT + 1);
        for body in [
            Bytes::from_static(br#"{"model":""}"#),
            Bytes::from_static(br#"{"model":" leading-space"}"#),
            Bytes::from_static(br#"{"model":"control\u0000character"}"#),
            Bytes::from(format!(r#"{{"model":"{over_limit}"}}"#)),
        ] {
            assert_eq!(
                collect_proxy_json_body(&json_headers(), Full::new(body))
                    .await
                    .unwrap_err(),
                OpenAiRequestError::InvalidModel
            );
        }
    }

    #[tokio::test]
    async fn proxy_validation_preserves_shape_and_syntax_errors() {
        for (body, expected) in [
            (br#"[]"#.as_slice(), OpenAiRequestError::InvalidBodyShape),
            (
                br#"{"model":"local""#.as_slice(),
                OpenAiRequestError::InvalidJson,
            ),
            (br#"{}"#.as_slice(), OpenAiRequestError::MissingModel),
            (
                br#"{"model":42}"#.as_slice(),
                OpenAiRequestError::InvalidModel,
            ),
        ] {
            let error =
                collect_proxy_json_body(&json_headers(), Full::new(Bytes::copy_from_slice(body)))
                    .await
                    .unwrap_err();
            assert_eq!(error, expected);
        }
    }

    #[tokio::test]
    async fn rewrites_only_the_model_for_managed_dispatch() {
        let request = collect_json_body(
            &json_headers(),
            Full::new(Bytes::from_static(
                br#"{ "model": "external", "messages": [{"role":"user","content":"hello"}] }"#,
            )),
        )
        .await
        .unwrap();

        let routed = request.routed_body("internal/model-v2", None).unwrap();
        let document: serde_json::Value = serde_json::from_slice(&routed).unwrap();
        assert_eq!(document["model"], "internal/model-v2");
        assert_eq!(document["messages"][0]["content"], "hello");
    }

    #[tokio::test]
    async fn managed_dispatch_collapses_duplicate_model_fields() {
        let request = collect_json_body(
            &json_headers(),
            Full::new(Bytes::from_static(
                br#"{"model":"hidden","model":"external","messages":[]}"#,
            )),
        )
        .await
        .unwrap();

        let routed = request.routed_body("external", None).unwrap();
        let routed = String::from_utf8(routed.to_vec()).unwrap();
        assert_eq!(routed.matches("\"model\"").count(), 1);
        assert_eq!(
            serde_json::from_str::<serde_json::Value>(&routed).unwrap()["model"],
            "external"
        );
    }

    #[tokio::test]
    async fn managed_dispatch_scopes_prompt_cache_affinity_without_forwarding_the_raw_key() {
        let request = collect_json_body(
            &json_headers(),
            Full::new(Bytes::from_static(
                br#"{"model":"external","prompt_cache_key":"private-session","messages":[]}"#,
            )),
        )
        .await
        .unwrap();
        let credential_id = uuid::Uuid::from_u128(1);
        let model_id = uuid::Uuid::from_u128(2);
        let scoped = request
            .scoped_prompt_cache_key(
                credential_id,
                model_id,
                crate::config::InferenceEndpoint::ChatCompletions,
            )
            .unwrap();
        assert_eq!(
            Some(scoped.clone()),
            request.scoped_prompt_cache_key(
                credential_id,
                model_id,
                crate::config::InferenceEndpoint::ChatCompletions,
            )
        );
        assert_ne!(
            Some(scoped.clone()),
            request.scoped_prompt_cache_key(
                uuid::Uuid::from_u128(3),
                model_id,
                crate::config::InferenceEndpoint::ChatCompletions,
            )
        );
        assert!(scoped.starts_with("a3s-gw-pcache-v1:"));
        assert!(!scoped.contains("private-session"));

        let routed = request.routed_body("internal", Some(&scoped)).unwrap();
        let document: serde_json::Value = serde_json::from_slice(&routed).unwrap();
        assert_eq!(document["prompt_cache_key"], scoped);
        assert!(!String::from_utf8(routed.to_vec())
            .unwrap()
            .contains("private-session"));
    }

    #[tokio::test]
    async fn rejects_ambiguous_or_unbounded_prompt_cache_keys() {
        let over_limit = "x".repeat(PROMPT_CACHE_KEY_LIMIT + 1);
        let bodies = [
            Bytes::from_static(br#"{"model":"external","prompt_cache_key":42}"#),
            Bytes::from_static(br#"{"model":"external","prompt_cache_key":""}"#),
            Bytes::from_static(br#"{"model":"external","prompt_cache_key":" leading"}"#),
            Bytes::from_static(br#"{"model":"external","prompt_cache_key":"line\nbreak"}"#),
            Bytes::from(format!(
                r#"{{"model":"external","prompt_cache_key":"{over_limit}"}}"#
            )),
        ];
        for body in bodies {
            assert_eq!(
                collect_json_body(&json_headers(), Full::new(body))
                    .await
                    .unwrap_err(),
                OpenAiRequestError::InvalidPromptCacheKey
            );
        }
    }

    #[tokio::test]
    async fn rejects_missing_or_non_json_content_type() {
        let body = || Full::new(Bytes::from_static(br#"{"model":"local"}"#));

        assert_eq!(
            collect_json_body(&HeaderMap::new(), body()).await,
            Err(OpenAiRequestError::UnsupportedMediaType)
        );

        let mut headers = HeaderMap::new();
        headers.insert(CONTENT_TYPE, "application/problem+json".parse().unwrap());
        assert_eq!(
            collect_json_body(&headers, body()).await,
            Err(OpenAiRequestError::UnsupportedMediaType)
        );
    }

    #[tokio::test]
    async fn enforces_the_observed_body_limit() {
        let headers = json_headers();
        let at_limit = Bytes::from(vec![b' '; OPENAI_REQUEST_BODY_LIMIT]);
        assert_eq!(
            collect_json_body(&headers, Full::new(at_limit)).await,
            Err(OpenAiRequestError::InvalidJson)
        );

        let over_limit = Bytes::from(vec![b' '; OPENAI_REQUEST_BODY_LIMIT + 1]);
        assert_eq!(
            collect_json_body(&headers, Full::new(over_limit)).await,
            Err(OpenAiRequestError::BodyTooLarge)
        );
    }

    #[tokio::test]
    async fn rejects_an_oversized_declared_length_before_reading() {
        let mut headers = json_headers();
        headers.insert(
            CONTENT_LENGTH,
            (OPENAI_REQUEST_BODY_LIMIT + 1).to_string().parse().unwrap(),
        );

        assert_eq!(
            collect_json_body(&headers, Full::new(Bytes::new())).await,
            Err(OpenAiRequestError::BodyTooLarge)
        );
    }

    #[tokio::test]
    async fn rejects_invalid_json_without_echoing_parser_details() {
        let error = collect_json_body(
            &json_headers(),
            Full::new(Bytes::from_static(br#"{"model":"local""#)),
        )
        .await
        .unwrap_err();
        let response = error.into_response();

        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        assert_eq!(response.headers()[CONTENT_TYPE], "application/json");
        let body: serde_json::Value = serde_json::from_slice(response.body().as_ref())
            .expect("error envelope must be valid JSON");
        assert_eq!(
            body["message"], "Request body must contain valid JSON.",
            "the stable message must survive the envelope projection"
        );
        assert_eq!(body["success"], false);
        assert_eq!(body["code"], 40000);
        let raw = String::from_utf8_lossy(response.body().as_ref());
        assert!(
            !raw.contains("serde") && !raw.contains("expected ident"),
            "parser internals must not be echoed: {raw}"
        );
    }

    #[tokio::test]
    async fn requires_a_json_object_with_a_bounded_model_alias() {
        let cases = [
            (br#"[]"#.as_slice(), OpenAiRequestError::InvalidBodyShape),
            (br#"{}"#.as_slice(), OpenAiRequestError::MissingModel),
            (
                br#"{"model":42}"#.as_slice(),
                OpenAiRequestError::InvalidModel,
            ),
            (
                br#"{"model":""}"#.as_slice(),
                OpenAiRequestError::InvalidModel,
            ),
            (
                br#"{"model":" leading-space"}"#.as_slice(),
                OpenAiRequestError::InvalidModel,
            ),
            (
                br#"{"model":"control\u0000character"}"#.as_slice(),
                OpenAiRequestError::InvalidModel,
            ),
        ];

        for (body, expected) in cases {
            assert_eq!(
                collect_json_body(&json_headers(), Full::new(Bytes::copy_from_slice(body)))
                    .await
                    .unwrap_err(),
                expected
            );
        }

        let at_limit = "x".repeat(OPENAI_MODEL_ALIAS_LIMIT);
        let body = Bytes::from(format!(r#"{{"model":"{at_limit}"}}"#));
        let request = collect_json_body(&json_headers(), Full::new(body))
            .await
            .unwrap();
        assert_eq!(request.model_alias(), at_limit);

        let over_limit = "x".repeat(OPENAI_MODEL_ALIAS_LIMIT + 1);
        let body = Bytes::from(format!(r#"{{"model":"{over_limit}"}}"#));
        assert_eq!(
            collect_json_body(&json_headers(), Full::new(body))
                .await
                .unwrap_err(),
            OpenAiRequestError::InvalidModel
        );

        let multi_byte_over_limit = "é".repeat(OPENAI_MODEL_ALIAS_LIMIT / 2 + 1);
        assert!(multi_byte_over_limit.len() > OPENAI_MODEL_ALIAS_LIMIT);
        let body = Bytes::from(format!(r#"{{"model":"{multi_byte_over_limit}"}}"#));
        assert_eq!(
            collect_json_body(&json_headers(), Full::new(body))
                .await
                .unwrap_err(),
            OpenAiRequestError::InvalidModel
        );
    }

    #[test]
    fn stable_error_contract_maps_each_failure() {
        let cases = [
            (
                OpenAiRequestError::UnsupportedMediaType,
                StatusCode::UNSUPPORTED_MEDIA_TYPE,
                "unsupported_media_type",
                None,
            ),
            (
                OpenAiRequestError::BodyTooLarge,
                StatusCode::PAYLOAD_TOO_LARGE,
                "request_too_large",
                None,
            ),
            (
                OpenAiRequestError::BodyReadFailed,
                StatusCode::BAD_REQUEST,
                "invalid_request_body",
                None,
            ),
            (
                OpenAiRequestError::InvalidJson,
                StatusCode::BAD_REQUEST,
                "invalid_json",
                None,
            ),
            (
                OpenAiRequestError::InvalidBodyShape,
                StatusCode::BAD_REQUEST,
                "invalid_request_body",
                None,
            ),
            (
                OpenAiRequestError::MissingModel,
                StatusCode::BAD_REQUEST,
                "missing_model",
                Some("model"),
            ),
            (
                OpenAiRequestError::InvalidModel,
                StatusCode::BAD_REQUEST,
                "invalid_model",
                Some("model"),
            ),
            (
                OpenAiRequestError::InvalidPromptCacheKey,
                StatusCode::BAD_REQUEST,
                "invalid_prompt_cache_key",
                Some("prompt_cache_key"),
            ),
        ];

        for (error, status, _code, _param) in cases {
            let response = error.into_response();
            let body: serde_json::Value = serde_json::from_slice(response.body()).unwrap();
            assert_eq!(response.status(), status);
            assert_eq!(body["success"], false);
            assert_eq!(body["error_code"], "request.invalid");
            assert_eq!(body["code"], 40000);
        }
    }
}
