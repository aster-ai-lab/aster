//! Request and response shapes with closed-set limits.
//!
//! The embedding shape follows the OpenAI embeddings endpoint; the rerank
//! shape follows the Jina/Cohere rerank wire used by A3S Studio. Unknown
//! fields are tolerated (both wire formats carry optional fields this
//! crate does not act on), but every field this crate does act on is
//! bounded and every refusal names its limit.

use serde::{Deserialize, Serialize};

use crate::UseError;

/// At most this many texts per request. Mirrors the rerank document cap so
/// both endpoints share one notion of "one bounded batch".
pub(crate) const MAX_ITEMS: usize = 256;
/// One input string may span at most this many UTF-8 bytes.
pub(crate) const MAX_ITEM_BYTES: usize = 64 * 1024;
/// The model alias is echoed, never routed on; bounded to reject dumping
/// arbitrary text through it.
pub(crate) const MAX_MODEL_BYTES: usize = 200;

#[derive(Debug, Clone, PartialEq)]
pub struct EmbeddingRequest {
    /// Echoed in the response; the engine always serves its installed family.
    pub model: Option<String>,
    pub input: Vec<String>,
}

impl EmbeddingRequest {
    /// Parses an OpenAI embeddings body. `input` accepts one string or an
    /// array of strings; token arrays and any other shape are refused.
    pub fn from_json(body: &[u8]) -> UseResultJson<Self> {
        #[derive(Deserialize)]
        struct Raw {
            #[serde(default)]
            model: Option<String>,
            input: serde_json::Value,
            #[serde(default)]
            encoding_format: Option<String>,
        }
        let raw: Raw = serde_json::from_slice(body).map_err(|error| {
            request_error(format!("the embedding request is not valid JSON: {error}"))
        })?;
        if let Some(model) = &raw.model {
            bounded(model.len(), MAX_MODEL_BYTES, "the model field").map_err(request_error)?;
        }
        if let Some(format) = &raw.encoding_format {
            if format != "float" {
                return Err(request_error(format!(
                    "encoding_format must be float, got {format:?}"
                )));
            }
        }
        let input = match raw.input {
            serde_json::Value::String(value) => vec![value],
            serde_json::Value::Array(items) => items
                .into_iter()
                .map(|item| match item {
                    serde_json::Value::String(value) => Ok(value),
                    _ => Err(request_error(
                        "every embedding input entry must be a string; token arrays are not supported.",
                    )),
                })
                .collect::<Result<Vec<_>, _>>()?,
            _ => {
                return Err(request_error(
                    "the embedding input must be a string or an array of strings.",
                ))
            }
        };
        if input.is_empty() || input.len() > MAX_ITEMS {
            return Err(request_error(format!(
                "the embedding input must hold 1-{MAX_ITEMS} strings, got {}.",
                input.len()
            )));
        }
        for value in &input {
            bounded(value.len(), MAX_ITEM_BYTES, "an embedding input entry")
                .map_err(request_error)?;
        }
        Ok(Self {
            model: raw.model,
            input,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct EmbeddingResponse {
    pub object: &'static str,
    pub data: Vec<EmbeddingItem>,
    pub model: String,
    pub usage: Usage,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct EmbeddingItem {
    pub object: &'static str,
    pub index: usize,
    pub embedding: Vec<f32>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct RerankRequest {
    pub model: Option<String>,
    pub query: String,
    pub documents: Vec<String>,
    pub top_n: Option<usize>,
}

impl RerankRequest {
    /// Parses a Jina/Cohere-shaped rerank body.
    pub fn from_json(body: &[u8]) -> UseResultJson<Self> {
        #[derive(Deserialize)]
        struct Raw {
            #[serde(default)]
            model: Option<String>,
            query: String,
            documents: Vec<String>,
            #[serde(default)]
            top_n: Option<serde_json::Value>,
        }
        let raw: Raw = serde_json::from_slice(body).map_err(|error| {
            request_error(format!("the rerank request is not valid JSON: {error}"))
        })?;
        if let Some(model) = &raw.model {
            bounded(model.len(), MAX_MODEL_BYTES, "the model field").map_err(request_error)?;
        }
        bounded(raw.query.len(), MAX_ITEM_BYTES, "the query").map_err(request_error)?;
        if raw.documents.is_empty() || raw.documents.len() > MAX_ITEMS {
            return Err(request_error(format!(
                "rerank documents must hold 1-{MAX_ITEMS} strings, got {}.",
                raw.documents.len()
            )));
        }
        for value in &raw.documents {
            bounded(value.len(), MAX_ITEM_BYTES, "a rerank document").map_err(request_error)?;
        }
        let top_n = match raw.top_n {
            None => None,
            Some(serde_json::Value::Null) => None,
            Some(serde_json::Value::Number(number)) => {
                let value = number
                    .as_u64()
                    .ok_or_else(|| request_error("top_n must be a non-negative integer."))?;
                if value == 0 {
                    return Err(request_error("top_n must be at least 1."));
                }
                let value = usize::try_from(value)
                    .map_err(|_| request_error("top_n exceeds the platform addressable range."))?;
                if value > raw.documents.len() {
                    return Err(request_error(format!(
                        "top_n must not exceed the {} provided documents.",
                        raw.documents.len()
                    )));
                }
                Some(value)
            }
            Some(_) => return Err(request_error("top_n must be a non-negative integer.")),
        };
        Ok(Self {
            model: raw.model,
            query: raw.query,
            documents: raw.documents,
            top_n,
        })
    }

    /// How many results the caller asked for; all documents when unset.
    pub fn effective_top_n(&self) -> usize {
        self.top_n.unwrap_or(self.documents.len())
    }
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct RerankResponse {
    pub model: String,
    pub results: Vec<RerankResult>,
    pub usage: Usage,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct RerankResult {
    pub index: usize,
    pub relevance_score: f32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct Usage {
    pub prompt_tokens: u64,
    pub total_tokens: u64,
}

fn bounded(len: usize, max: usize, what: &str) -> Result<(), String> {
    if len == 0 {
        Err(format!("{what} must not be empty."))
    } else if len > max {
        Err(format!("{what} exceeds the {max}-byte limit."))
    } else {
        Ok(())
    }
}

fn request_error(message: impl Into<String>) -> UseError {
    UseError::new("use.bge.request_invalid", message)
}

/// Result alias keeping the wire module's error construction private.
pub type UseResultJson<T> = Result<T, UseError>;
