//! Client for the Aster gateway search service at `POST /v1/search`.
//!
//! The gateway forwards that route to `aster-search serve`, which answers from
//! Wikipedia, DuckDuckGo, Brave, and Bing. This provider does not send
//! Firecrawl-only filters.

use async_trait::async_trait;
use serde::Deserialize;
use serde::Serialize;
use serde_json::Value;

use super::http::ProviderHttpClient;
use super::json_api::{invalid_config, JsonAuth, JsonSearchApi};
use super::protocol::{
    non_empty, sanitize_provider_multiline_text, sanitize_provider_text_with_secrets,
    strip_simple_markup, validated_web_url,
};
use super::{
    CredentialSource, ProviderCapabilities, ProviderDescriptor, ProviderHttpConfig,
    ProviderReadiness, ProviderReport, ProviderRequest, ProviderResponse, ProviderResult,
    SearchProvider,
};
use crate::{ProviderErrorKind, Result, ResultType, SearchUsage};
use url::Url;

const PROVIDER_ID: &str = "aster";
const DEFAULT_ENDPOINT: &str = "http://127.0.0.1:8080/v1/search";
const MAX_QUERY_CHARS: usize = 500;
const MAX_LIMIT: u8 = 100;

/// How Aster expects the caller credential.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum AsterAuth {
    /// `Authorization: Bearer <key>`.
    Bearer,
    /// `X-API-Key: <key>`.
    ApiKey,
}

/// Typed Aster search defaults and credentials.
#[derive(Debug, Clone)]
pub struct AsterConfig {
    api: JsonSearchApi,
    max_results: u8,
    include_markdown: bool,
}

impl AsterConfig {
    /// Creates the default Aster configuration.
    ///
    /// `ASTER_GATEWAY_KEY` is required. The default request asks for web metadata
    /// only. Markdown scraping is opt-in.
    pub fn new() -> Result<Self> {
        Ok(Self {
            api: JsonSearchApi::new(PROVIDER_ID, "Aster", DEFAULT_ENDPOINT, "ASTER_GATEWAY_KEY")?,
            max_results: 10,
            include_markdown: false,
        })
    }

    /// Replaces the API endpoint.
    pub fn with_endpoint(mut self, endpoint: Url) -> Result<Self> {
        self.api = self.api.with_endpoint(endpoint)?;
        Ok(self)
    }

    /// Replaces the API-key source.
    pub fn with_api_key(mut self, api_key: CredentialSource) -> Self {
        self.api = self.api.with_api_key(api_key);
        self
    }

    /// Selects Bearer or `X-API-Key` authentication.
    pub fn with_auth(mut self, auth: AsterAuth) -> Self {
        let wire = match auth {
            AsterAuth::Bearer => JsonAuth::Bearer,
            AsterAuth::ApiKey => JsonAuth::Header("x-api-key"),
        };
        self.api = self.api.with_auth(wire);
        self
    }

    /// Sets the result cap in the `1..=100` range.
    ///
    /// The Aster wire field remains `limit`.
    pub fn with_max_results(mut self, max_results: u8) -> Result<Self> {
        if !(1..=MAX_LIMIT).contains(&max_results) {
            return Err(invalid_config(
                PROVIDER_ID,
                "Aster max_results must be between 1 and 100",
            ));
        }
        self.max_results = max_results;
        Ok(self)
    }

    /// Requests markdown page text for each result.
    pub fn with_include_markdown(mut self, include_markdown: bool) -> Self {
        self.include_markdown = include_markdown;
        self
    }

    /// Replaces HTTP transport limits.
    pub fn with_http_config(mut self, http: ProviderHttpConfig) -> Self {
        self.api = self.api.with_http_config(http);
        self
    }

    /// Returns the configured endpoint.
    pub fn endpoint(&self) -> &Url {
        self.api.endpoint()
    }
}

/// Native Rust implementation of the Aster search API.
#[derive(Debug)]
pub struct AsterProvider {
    config: AsterConfig,
    client: ProviderHttpClient,
}

impl AsterProvider {
    /// Creates a provider from typed configuration.
    pub fn new(config: AsterConfig) -> Result<Self> {
        let client = config.api.client()?;
        Ok(Self { config, client })
    }

    /// Creates a provider using `ASTER_GATEWAY_KEY`.
    pub fn from_env() -> Result<Self> {
        Self::new(AsterConfig::new()?)
    }
}

#[async_trait]
impl SearchProvider for AsterProvider {
    fn descriptor(&self) -> ProviderDescriptor {
        let mut capabilities = ProviderCapabilities::new().with_usage(true);
        if self.config.include_markdown {
            capabilities = capabilities.with_full_text(true);
        }
        ProviderDescriptor::new(
            PROVIDER_ID,
            "Aster",
            "https://github.com/aster-ai-lab/Search",
            capabilities,
        )
    }

    fn readiness(&self) -> ProviderReadiness {
        self.config.api.readiness()
    }

    async fn search(&self, request: &ProviderRequest) -> Result<ProviderResponse> {
        let query = self.config.api.bounded_query(request, MAX_QUERY_CHARS)?;
        let payload = AsterRequest {
            query,
            limit: self.max_results_limit(),
            sources: ["web"],
            scrape_options: self.config.include_markdown.then_some(AsterScrapeOptions {
                formats: [AsterFormat { kind: "markdown" }],
            }),
        };
        let reply = self.config.api.post_json(&self.client, &payload).await?;
        let secrets = reply.secrets();
        if !reply.is_success() {
            return Err(reply.reject("Aster request failed", |code, message, _| {
                aster_kind(code, message)
            }));
        }

        let payload: Value = reply.decode()?;
        match payload.get("success") {
            Some(Value::Bool(false)) => {
                return Err(
                    reply.reject("Aster rejected the request", |code, message, _| {
                        aster_kind(code, message).or(Some(ProviderErrorKind::InvalidRequest))
                    }),
                );
            }
            Some(Value::Bool(true)) => {}
            _ => return Err(reply.contract_error()),
        }
        let document: AsterDocument =
            serde_json::from_value(payload).map_err(|_| reply.contract_error())?;
        let mut results = Vec::new();
        for hit in document.data.hits() {
            if let Some(result) = adapt_web(hit, &secrets) {
                results.push(result);
            }
        }
        results.truncate(usize::from(self.max_results_limit()));
        let request_id = document
            .id
            .map(|value| sanitize_provider_text_with_secrets(&value, 128, &secrets))
            .filter(|value| !value.is_empty());
        Ok(reply.seal(ProviderResponse {
            results,
            report: ProviderReport {
                request_id,
                usage: document.credits_used.and_then(usage_from_credits),
                ..Default::default()
            },
            ..Default::default()
        }))
    }
}

impl AsterProvider {
    fn max_results_limit(&self) -> u8 {
        self.config.max_results
    }
}

#[derive(Serialize)]
struct AsterRequest<'a> {
    query: &'a str,
    limit: u8,
    sources: [&'static str; 1],
    #[serde(rename = "scrapeOptions", skip_serializing_if = "Option::is_none")]
    scrape_options: Option<AsterScrapeOptions>,
}

#[derive(Serialize)]
struct AsterScrapeOptions {
    formats: [AsterFormat; 1],
}

#[derive(Serialize)]
struct AsterFormat {
    #[serde(rename = "type")]
    kind: &'static str,
}

#[derive(Deserialize)]
struct AsterDocument {
    #[serde(default)]
    data: AsterData,
    id: Option<String>,
    #[serde(rename = "creditsUsed")]
    credits_used: Option<f64>,
}

#[derive(Deserialize, Default)]
struct AsterData {
    #[serde(default)]
    web: Vec<AsterWebHit>,
    #[serde(default)]
    results: Option<AsterResults>,
}

impl AsterData {
    fn hits(self) -> Vec<AsterWebHit> {
        let mut hits = self.web;
        match self.results {
            Some(AsterResults::List(list)) => hits.extend(list),
            Some(AsterResults::Grouped { web }) => hits.extend(web),
            None => {}
        }
        hits
    }
}

/// `POST /v1/search` puts hits in `data.results`. With `sources` set, Aster
/// sends the grouped object; without sources it sends a flat list.
#[derive(Deserialize)]
#[serde(untagged)]
enum AsterResults {
    List(Vec<AsterWebHit>),
    Grouped {
        #[serde(default)]
        web: Vec<AsterWebHit>,
    },
}

#[derive(Deserialize)]
struct AsterWebHit {
    title: Option<String>,
    description: Option<String>,
    url: Option<String>,
    markdown: Option<String>,
}

fn adapt_web(hit: AsterWebHit, secrets: &[&str]) -> Option<ProviderResult> {
    let url = hit.url.as_deref().and_then(validated_web_url)?;
    let title = titled(hit.title.as_deref(), &url, secrets);
    let snippet = JsonSearchApi::clean(hit.description.as_deref().unwrap_or(""), 2_000, secrets);
    let mut result = ProviderResult::new(url, title, snippet).with_result_type(ResultType::Web);
    if let Some(markdown) = non_empty(hit.markdown) {
        result = attach_full_text(result, &markdown, secrets);
    }
    Some(result)
}

fn attach_full_text(result: ProviderResult, markdown: &str, secrets: &[&str]) -> ProviderResult {
    let text =
        sanitize_provider_text_with_secrets(&strip_simple_markup(markdown), 16 * 1024, secrets);
    let text = sanitize_provider_multiline_text(&text, 16 * 1024);
    if text.is_empty() {
        result
    } else {
        result.with_full_text(text)
    }
}

fn titled(title: Option<&str>, url: &str, secrets: &[&str]) -> String {
    let title = JsonSearchApi::clean(title.unwrap_or(url), 300, secrets);
    if title.is_empty() {
        url.to_string()
    } else {
        title
    }
}

fn usage_from_credits(credits: f64) -> Option<SearchUsage> {
    (credits.is_finite() && credits >= 0.0).then(|| SearchUsage::new().with_credits(credits))
}

fn aster_kind(code: Option<&str>, message: Option<&str>) -> Option<ProviderErrorKind> {
    match code.unwrap_or("") {
        "UNAUTHORIZED" | "INVALID_API_KEY" | "MISSING_API_KEY" => {
            Some(ProviderErrorKind::Authentication)
        }
        "PAYMENT_REQUIRED" | "INSUFFICIENT_CREDITS" | "QUOTA_EXCEEDED" => {
            Some(ProviderErrorKind::Quota)
        }
        "RATE_LIMIT_EXCEEDED" | "RATE_LIMITED" => Some(ProviderErrorKind::RateLimited),
        "FORBIDDEN" => Some(ProviderErrorKind::Permission),
        _ if message.is_some_and(mentions_quota) => Some(ProviderErrorKind::Quota),
        _ => None,
    }
}

fn mentions_quota(message: &str) -> bool {
    let message = message.to_ascii_lowercase();
    message.contains("credit") || message.contains("quota") || message.contains("payment required")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn max_results_rejects_values_outside_the_vendor_range() {
        assert!(AsterConfig::new().unwrap().with_max_results(0).is_err());
        assert!(AsterConfig::new().unwrap().with_max_results(101).is_err());
        assert!(AsterConfig::new().unwrap().with_max_results(10).is_ok());
    }
}
