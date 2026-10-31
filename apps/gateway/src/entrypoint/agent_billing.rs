//! Charge successful `/v1/search` and `/v1/parse` calls through aster-billing.
//!
//! The listener reads `ASTER_BILLING_URL` and `ASTER_BILLING_INTERNAL_TOKEN` once
//! at startup. A router that publishes search or parse refuses to start unless
//! both are set. One set and the other empty also refuses startup.

use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

/// Largest page window the parse service accepts.
pub(crate) const MAX_PARSED_PAGES: i64 = 500;
const IDEMPOTENCY_TTL: Duration = Duration::from_secs(24 * 60 * 60);
const MAX_STORED_RESPONSE_BYTES: usize = 8 * 1024 * 1024;
const MAX_STORED_RESPONSE_TOTAL: usize = 64 * 1024 * 1024;

#[derive(Clone)]
pub(crate) struct AgentBilling {
    base: String,
    token: String,
    http: reqwest::Client,
    ensured: Arc<Mutex<HashSet<String>>>,
    idempotency: Arc<Mutex<HashMap<String, IdempotencyRecord>>>,
}

impl AgentBilling {
    fn new(base: String, token: String, http: reqwest::Client) -> Self {
        Self {
            base,
            token,
            http,
            ensured: Arc::new(Mutex::new(HashSet::new())),
            idempotency: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    #[cfg(test)]
    pub(crate) fn with_endpoint(base: String) -> Self {
        Self::new(
            base,
            "billing-test".into(),
            reqwest::Client::builder()
                .timeout(Duration::from_secs(3))
                .build()
                .expect("test client"),
        )
    }

    pub(crate) fn from_env() -> Result<Option<Self>, String> {
        let base = std::env::var("ASTER_BILLING_URL")
            .ok()
            .map(|value| value.trim().trim_end_matches('/').to_string())
            .filter(|value| !value.is_empty());
        let token = std::env::var("ASTER_BILLING_INTERNAL_TOKEN")
            .ok()
            .filter(|value| !value.is_empty());
        match (base, token) {
            (None, None) => Ok(None),
            (Some(base), Some(token)) => Ok(Some(Self::new(
                base,
                token,
                reqwest::Client::builder()
                    .timeout(Duration::from_secs(3))
                    .build()
                    .map_err(|error| error.to_string())?,
            ))),
            _ => Err(
                "ASTER_BILLING_URL and ASTER_BILLING_INTERNAL_TOKEN must both be set to meter /v1/search and /v1/parse"
                    .into(),
            ),
        }
    }

    pub(crate) async fn charge(
        &self,
        org_id: &str,
        code: &str,
        field: &str,
        quantity: i64,
        transaction_id: &str,
    ) -> Result<(), String> {
        if !lock_mutex(&self.ensured).contains(org_id) {
            self.post(
                "/v1/agent-subscriptions",
                serde_json::json!({ "org_id": org_id }),
            )
            .await?;
            lock_mutex(&self.ensured).insert(org_id.to_owned());
        }
        if let Err(error) = self
            .post(
                "/v1/events",
                serde_json::json!({
                    "transaction_id": transaction_id,
                    "external_subscription_id": format!("agent:{org_id}"),
                    "code": code,
                    "properties": { field: quantity },
                }),
            )
            .await
        {
            lock_mutex(&self.ensured).remove(org_id);
            return Err(error);
        }
        Ok(())
    }

    pub(crate) fn accept_stored_response(&self, body_len: usize) -> Result<(), &'static str> {
        if body_len > MAX_STORED_RESPONSE_BYTES {
            return Err("idempotency response is too large");
        }
        let store = lock_mutex(&self.idempotency);
        let used: usize = store.values().map(|record| record.stored_len()).sum();
        if used.saturating_add(body_len) > MAX_STORED_RESPONSE_TOTAL {
            return Err("idempotency store is full");
        }
        Ok(())
    }

    async fn post(&self, path: &str, body: serde_json::Value) -> Result<(), String> {
        let response = self
            .http
            .post(format!("{}{path}", self.base))
            .bearer_auth(&self.token)
            .json(&body)
            .send()
            .await
            .map_err(|error| error.to_string())?;
        let status = response.status();
        let text = response.text().await.map_err(|error| error.to_string())?;
        if !status.is_success() {
            return Err(format!("billing {path} returned {status}: {text}"));
        }
        let envelope: serde_json::Value =
            serde_json::from_str(&text).map_err(|error| error.to_string())?;
        if envelope.get("success").and_then(|value| value.as_bool()) != Some(true) {
            return Err(format!("billing {path} rejected the charge: {text}"));
        }
        Ok(())
    }

    pub(crate) fn begin_idempotency(
        &self,
        org_id: &str,
        path: &str,
        code: &str,
        key: &str,
        digest: &[u8; 32],
        now: SystemTime,
    ) -> Idempotency {
        let mut store = lock_mutex(&self.idempotency);
        store.retain(|_, record| !idempotency_expired(record.stored_at, now));
        let storage_key = format!("{org_id}\n{path}\n{key}");
        if let Some(record) = store.get_mut(&storage_key) {
            if record.digest != *digest {
                return Idempotency::Rejected("idempotency key does not match this request");
            }
            if let Some(response) = record.response.clone() {
                return Idempotency::Replay(response);
            }
            if record.retryable {
                record.retryable = false;
                return Idempotency::Proceed {
                    transaction_id: record.transaction_id.clone(),
                };
            }
            return Idempotency::Rejected("idempotency key is in progress");
        }
        let used: usize = store.values().map(|record| record.stored_len()).sum();
        if used >= MAX_STORED_RESPONSE_TOTAL {
            return Idempotency::Rejected("idempotency store is full");
        }
        let stored_at = now;
        let transaction_id = format!(
            "{code}:{org_id}:{key}:{}",
            stored_at
                .duration_since(UNIX_EPOCH)
                .map(|duration| duration.as_secs())
                .unwrap_or(0)
        );
        store.insert(
            storage_key,
            IdempotencyRecord {
                digest: *digest,
                stored_at,
                transaction_id: transaction_id.clone(),
                retryable: false,
                response: None,
            },
        );
        Idempotency::Proceed { transaction_id }
    }

    pub(crate) fn complete_idempotency(
        &self,
        org_id: &str,
        path: &str,
        key: &str,
        digest: &[u8; 32],
        response: StoredResponse,
    ) -> Result<(), &'static str> {
        if response.body.len() > MAX_STORED_RESPONSE_BYTES {
            return Err("idempotency response is too large");
        }
        let mut store = lock_mutex(&self.idempotency);
        let Some(record) = store.get_mut(&format!("{org_id}\n{path}\n{key}")) else {
            return Err("idempotency key is not in progress");
        };
        if record.digest != *digest || record.response.is_some() {
            return Err("idempotency key is not in progress");
        }
        record.response = Some(response);
        Ok(())
    }

    pub(crate) fn lease<'a>(
        &'a self,
        org_id: &'a str,
        path: &'a str,
        key: &'a str,
    ) -> IdempotencyLease<'a> {
        IdempotencyLease {
            billing: self,
            org_id,
            path,
            key,
            armed: true,
        }
    }

    pub(crate) fn mark_retryable(&self, org_id: &str, path: &str, key: &str) {
        let mut store = lock_mutex(&self.idempotency);
        if let Some(record) = store.get_mut(&format!("{org_id}\n{path}\n{key}")) {
            if record.response.is_none() {
                record.retryable = true;
            }
        }
    }

    pub(crate) fn cancel_idempotency(&self, org_id: &str, path: &str, key: &str) {
        let mut store = lock_mutex(&self.idempotency);
        let storage_key = format!("{org_id}\n{path}\n{key}");
        if store
            .get(&storage_key)
            .is_some_and(|record| record.response.is_none())
        {
            store.remove(&storage_key);
        }
    }
}

const ORG_REQUIRED: &str = "search and parse require an organization";

#[derive(Clone)]
struct IdempotencyRecord {
    digest: [u8; 32],
    stored_at: SystemTime,
    transaction_id: String,
    retryable: bool,
    response: Option<StoredResponse>,
}

impl IdempotencyRecord {
    fn stored_len(&self) -> usize {
        self.response
            .as_ref()
            .map(|response| response.body.len())
            .unwrap_or(0)
    }
}

#[derive(Clone, Debug)]
pub(crate) struct StoredResponse {
    pub status: u16,
    pub content_type: Option<String>,
    pub body: Vec<u8>,
}

pub(crate) struct IdempotencyLease<'a> {
    billing: &'a AgentBilling,
    org_id: &'a str,
    path: &'a str,
    key: &'a str,
    armed: bool,
}

impl IdempotencyLease<'_> {
    pub(crate) fn disarm(&mut self) {
        self.armed = false;
    }

    pub(crate) fn release_for_retry(&mut self) {
        self.billing
            .mark_retryable(self.org_id, self.path, self.key);
        self.armed = false;
    }
}

/// A charge that already landed must keep its transaction id when the saved
/// response does not. Dropping the lease otherwise deletes the in-progress row.
pub(crate) fn retain_idempotency_after_charge(lease: &mut IdempotencyLease<'_>, stored: bool) {
    if stored {
        lease.disarm();
    } else {
        lease.release_for_retry();
    }
}

impl Drop for IdempotencyLease<'_> {
    fn drop(&mut self) {
        if self.armed {
            self.billing
                .cancel_idempotency(self.org_id, self.path, self.key);
        }
    }
}

#[derive(Debug)]
pub(crate) enum Idempotency {
    Proceed { transaction_id: String },
    Replay(StoredResponse),
    Rejected(&'static str),
}

pub(crate) fn request_digest(method: &str, path: &str, body: &[u8]) -> [u8; 32] {
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    hasher.update(method.as_bytes());
    hasher.update(b"\n");
    hasher.update(path.as_bytes());
    hasher.update(b"\n");
    hasher.update(body);
    hasher.finalize().into()
}

fn idempotency_expired(stored_at: SystemTime, now: SystemTime) -> bool {
    now.duration_since(stored_at)
        .is_ok_and(|age| age >= IDEMPOTENCY_TTL)
}

fn lock_mutex<T>(mutex: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    mutex
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

pub(crate) fn meter(service: &str, path: &str) -> Option<(&'static str, &'static str)> {
    match (service, path) {
        ("search", "/v1/search") => Some(("search_request", "requests")),
        ("parse", "/v1/parse") => Some(("parsed_page", "pages")),
        _ => None,
    }
}

/// Refuse a metered call before the gateway opens the upstream.
/// Billing that is unset leaves the route open.
pub(crate) fn refusal(
    billing: Option<&AgentBilling>,
    service: &str,
    path: &str,
    headers: &http::HeaderMap,
) -> Option<&'static str> {
    if billing.is_none() || meter(service, path).is_none() {
        return None;
    }
    billable_org(headers).err().map(|_| ORG_REQUIRED)
}

/// Record one successful metered call, then drop the internal quantity header.
/// A missing billing client leaves the route unmetered and still drops the header.
pub(crate) async fn settle(
    billing: Option<&AgentBilling>,
    service: &str,
    path: &str,
    status: http::StatusCode,
    response_headers: &mut http::HeaderMap,
    request_headers: &http::HeaderMap,
    transaction_id: Option<&str>,
) -> Result<(), String> {
    let outcome = match (billing, meter(service, path)) {
        (Some(billing), Some((code, field))) if status.is_success() => {
            let org_id = match billable_org(request_headers) {
                Ok(org_id) => org_id,
                Err(error) => {
                    response_headers.remove("x-aster-usage-quantity");
                    return Err(error);
                }
            };
            let quantity = if code == "parsed_page" {
                match usage_quantity(response_headers, true) {
                    Ok(quantity) if quantity > MAX_PARSED_PAGES => {
                        response_headers.remove("x-aster-usage-quantity");
                        return Err(format!("parsed page quantity exceeds {MAX_PARSED_PAGES}"));
                    }
                    Ok(quantity) => quantity,
                    Err(error) => {
                        response_headers.remove("x-aster-usage-quantity");
                        return Err(error);
                    }
                }
            } else {
                1
            };
            if quantity == 0 {
                Ok(())
            } else {
                let transaction_id = transaction_id
                    .map(str::to_owned)
                    .unwrap_or_else(|| format!("{code}:{org_id}:{}", uuid::Uuid::new_v4()));
                billing
                    .charge(org_id, code, field, quantity, &transaction_id)
                    .await
            }
        }
        _ => Ok(()),
    };
    response_headers.remove("x-aster-usage-quantity");
    outcome
}

fn billable_org(headers: &http::HeaderMap) -> Result<&str, String> {
    let org_id = header_text(headers, "x-aster-org").ok_or_else(|| ORG_REQUIRED.to_string())?;
    if uuid::Uuid::parse_str(org_id).is_err() {
        return Err(ORG_REQUIRED.to_string());
    }
    Ok(org_id)
}

fn header_text<'a>(headers: &'a http::HeaderMap, name: &str) -> Option<&'a str> {
    headers
        .get(name)
        .and_then(|value| value.to_str().ok())
        .map(str::trim)
        .filter(|value| !value.is_empty())
}

pub(crate) fn usage_quantity(
    headers: &http::HeaderMap,
    require_header: bool,
) -> Result<i64, String> {
    match headers.get("x-aster-usage-quantity") {
        Some(value) => {
            let text = value
                .to_str()
                .map_err(|_| "usage quantity is not text".to_string())?;
            let quantity: i64 = text
                .parse()
                .map_err(|_| "usage quantity is not an integer".to_string())?;
            if quantity < 0 {
                return Err("usage quantity must be zero or greater".into());
            }
            Ok(quantity)
        }
        None if require_header => Err("parse response omitted the usage quantity".into()),
        None => Ok(1),
    }
}

pub(crate) fn routes_need_agent_billing<'a>(rules: impl IntoIterator<Item = &'a str>) -> bool {
    rules
        .into_iter()
        .any(|rule| rule.contains("/v1/search") || rule.contains("/v1/parse"))
}

pub(crate) fn require_billing_for_agent_routes(
    needed: bool,
    billing: Option<AgentBilling>,
) -> Result<Option<AgentBilling>, String> {
    if needed && billing.is_none() {
        Err(
            "ASTER_BILLING_URL and ASTER_BILLING_INTERNAL_TOKEN are required when /v1/search or /v1/parse is routed"
                .into(),
        )
    } else {
        Ok(billing)
    }
}

/// A stored idempotent response is final. A server or rate-limit failure can
/// succeed on a later attempt, so the key stays available.
pub(crate) fn remember_idempotent_response(status: http::StatusCode) -> bool {
    !status.is_server_error()
        && status != http::StatusCode::TOO_MANY_REQUESTS
        && status != http::StatusCode::REQUEST_TIMEOUT
}

pub(crate) fn idempotent_agent_request(
    billing: Option<&AgentBilling>,
    service: &str,
    path: &str,
    headers: &http::HeaderMap,
) -> bool {
    billing.is_some() && meter(service, path).is_some() && headers.contains_key("idempotency-key")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn search_and_parse_are_the_metered_routes() {
        assert_eq!(
            meter("search", "/v1/search"),
            Some(("search_request", "requests"))
        );
        assert_eq!(meter("parse", "/v1/parse"), Some(("parsed_page", "pages")));
        assert_eq!(meter("search", "/health"), None);
        assert_eq!(meter("boyue", "/v1/search"), None);
    }

    #[test]
    fn parse_requires_a_positive_quantity_header() {
        let mut headers = http::HeaderMap::new();
        assert!(usage_quantity(&headers, true).is_err());
        headers.insert("x-aster-usage-quantity", http::HeaderValue::from_static("0"));
        assert_eq!(usage_quantity(&headers, true).unwrap(), 0);
        headers.insert("x-aster-usage-quantity", http::HeaderValue::from_static("4"));
        assert_eq!(usage_quantity(&headers, true).unwrap(), 4);
        assert_eq!(usage_quantity(&http::HeaderMap::new(), false).unwrap(), 1);
    }

    #[tokio::test]
    async fn unset_billing_drops_the_quantity_header() {
        let mut headers = http::HeaderMap::new();
        headers.insert("x-aster-usage-quantity", http::HeaderValue::from_static("2"));
        settle(
            None,
            "parse",
            "/v1/parse",
            http::StatusCode::OK,
            &mut headers,
            &http::HeaderMap::new(),
            None,
        )
        .await
        .unwrap();
        assert!(headers.get("x-aster-usage-quantity").is_none());
    }

    #[tokio::test]
    async fn a_missing_org_rejects_the_successful_response() {
        let billing = AgentBilling::with_endpoint("http://127.0.0.1:9".into());
        let error = settle(
            Some(&billing),
            "search",
            "/v1/search",
            http::StatusCode::OK,
            &mut http::HeaderMap::new(),
            &http::HeaderMap::new(),
            None,
        )
        .await
        .unwrap_err();
        assert!(error.contains("organization"), "{error}");
    }

    #[test]
    fn a_metered_call_without_an_organization_is_refused_before_upstream() {
        let billing = AgentBilling::with_endpoint("http://127.0.0.1:9".into());
        assert_eq!(
            refusal(
                Some(&billing),
                "search",
                "/v1/search",
                &http::HeaderMap::new()
            ),
            Some(ORG_REQUIRED)
        );
        let mut headers = http::HeaderMap::new();
        headers.insert("x-aster-org", http::HeaderValue::from_static("not-a-uuid"));
        assert_eq!(
            refusal(Some(&billing), "parse", "/v1/parse", &headers),
            Some(ORG_REQUIRED)
        );
        headers.insert(
            "x-aster-org",
            http::HeaderValue::from_static("11111111-1111-1111-1111-111111111111"),
        );
        assert!(refusal(Some(&billing), "search", "/v1/search", &headers).is_none());
        assert!(refusal(None, "search", "/v1/search", &http::HeaderMap::new()).is_none());
        assert!(refusal(
            Some(&billing),
            "boyue",
            "/v1/search",
            &http::HeaderMap::new()
        )
        .is_none());
    }

    #[tokio::test]
    async fn a_zero_page_parse_is_delivered_without_a_charge() {
        let billing = AgentBilling::with_endpoint("http://127.0.0.1:9".into());
        let mut request = http::HeaderMap::new();
        request.insert(
            "x-aster-org",
            http::HeaderValue::from_static("11111111-1111-1111-1111-111111111111"),
        );
        let mut response = http::HeaderMap::new();
        response.insert("x-aster-usage-quantity", http::HeaderValue::from_static("0"));
        settle(
            Some(&billing),
            "parse",
            "/v1/parse",
            http::StatusCode::OK,
            &mut response,
            &request,
            None,
        )
        .await
        .unwrap();
        assert!(response.get("x-aster-usage-quantity").is_none());
    }

    #[tokio::test]
    async fn each_successful_call_gets_a_new_transaction_id() {
        let seen = std::sync::Arc::new(tokio::sync::Mutex::new(Vec::<String>::new()));
        let base = accept_charges(std::sync::Arc::clone(&seen)).await;
        let billing = AgentBilling::with_endpoint(base);
        let mut request = http::HeaderMap::new();
        request.insert(
            "x-aster-org",
            http::HeaderValue::from_static("11111111-1111-1111-1111-111111111111"),
        );
        request.insert("x-request-id", http::HeaderValue::from_static("repeat-me"));
        for _ in 0..2 {
            settle(
                Some(&billing),
                "search",
                "/v1/search",
                http::StatusCode::OK,
                &mut http::HeaderMap::new(),
                &request,
                None,
            )
            .await
            .unwrap();
        }
        let bodies = seen.lock().await;
        let events: Vec<_> = bodies
            .iter()
            .filter(|body| body.contains("transaction_id"))
            .collect();
        assert_eq!(events.len(), 2);
        let ids: Vec<String> = events
            .iter()
            .map(|body| {
                serde_json::from_str::<serde_json::Value>(body)
                    .unwrap()
                    .get("transaction_id")
                    .and_then(|value| value.as_str())
                    .unwrap()
                    .to_string()
            })
            .collect();
        assert_ne!(ids[0], ids[1]);
        assert!(ids.iter().all(|id| !id.contains("repeat-me")));
    }

    #[tokio::test]
    async fn a_search_quantity_header_does_not_change_the_charge() {
        let seen = std::sync::Arc::new(tokio::sync::Mutex::new(Vec::<String>::new()));
        let base = accept_charges(std::sync::Arc::clone(&seen)).await;
        let billing = AgentBilling::with_endpoint(base);
        let mut request = http::HeaderMap::new();
        request.insert(
            "x-aster-org",
            http::HeaderValue::from_static("11111111-1111-1111-1111-111111111111"),
        );
        let mut response = http::HeaderMap::new();
        response.insert("x-aster-usage-quantity", http::HeaderValue::from_static("9"));
        settle(
            Some(&billing),
            "search",
            "/v1/search",
            http::StatusCode::OK,
            &mut response,
            &request,
            None,
        )
        .await
        .unwrap();
        assert!(response.get("x-aster-usage-quantity").is_none());
        let bodies = seen.lock().await;
        assert!(
            bodies.iter().any(|body| body.contains("\"requests\":1")),
            "{bodies:?}"
        );
        assert!(bodies.iter().all(|body| !body.contains("\"requests\":9")));
    }

    #[tokio::test]
    async fn parsed_pages_above_the_window_are_refused() {
        let billing = AgentBilling::with_endpoint("http://127.0.0.1:9".into());
        let mut request = http::HeaderMap::new();
        request.insert(
            "x-aster-org",
            http::HeaderValue::from_static("11111111-1111-1111-1111-111111111111"),
        );
        let mut response = http::HeaderMap::new();
        response.insert(
            "x-aster-usage-quantity",
            http::HeaderValue::from_static("501"),
        );
        let error = settle(
            Some(&billing),
            "parse",
            "/v1/parse",
            http::StatusCode::OK,
            &mut response,
            &request,
            None,
        )
        .await
        .unwrap_err();
        assert!(error.contains("500"), "{error}");
        assert!(response.get("x-aster-usage-quantity").is_none());
    }

    #[tokio::test]
    async fn the_full_page_window_is_charged() {
        let seen = std::sync::Arc::new(tokio::sync::Mutex::new(Vec::<String>::new()));
        let base = accept_charges(std::sync::Arc::clone(&seen)).await;
        let billing = AgentBilling::with_endpoint(base);
        let mut request = http::HeaderMap::new();
        request.insert(
            "x-aster-org",
            http::HeaderValue::from_static("11111111-1111-1111-1111-111111111111"),
        );
        let mut response = http::HeaderMap::new();
        response.insert(
            "x-aster-usage-quantity",
            http::HeaderValue::from_static("500"),
        );
        settle(
            Some(&billing),
            "parse",
            "/v1/parse",
            http::StatusCode::OK,
            &mut response,
            &request,
            None,
        )
        .await
        .unwrap();
        let bodies = seen.lock().await;
        assert!(
            bodies.iter().any(|body| body.contains("\"pages\":500")),
            "{bodies:?}"
        );
    }

    #[tokio::test]
    async fn a_subscription_is_ensured_once_per_organization() {
        let seen = std::sync::Arc::new(tokio::sync::Mutex::new(Vec::<String>::new()));
        let base = accept_charges(std::sync::Arc::clone(&seen)).await;
        let billing = AgentBilling::with_endpoint(base);
        let mut request = http::HeaderMap::new();
        request.insert(
            "x-aster-org",
            http::HeaderValue::from_static("11111111-1111-1111-1111-111111111111"),
        );
        for _ in 0..2 {
            settle(
                Some(&billing),
                "search",
                "/v1/search",
                http::StatusCode::OK,
                &mut http::HeaderMap::new(),
                &request,
                None,
            )
            .await
            .unwrap();
        }
        let bodies = seen.lock().await;
        let subscriptions = bodies.iter().filter(|body| body.contains("org_id")).count();
        let events = bodies
            .iter()
            .filter(|body| body.contains("transaction_id"))
            .count();
        assert_eq!(subscriptions, 1, "{bodies:?}");
        assert_eq!(events, 2, "{bodies:?}");
    }

    #[test]
    fn the_same_idempotency_key_replays_and_a_different_digest_is_rejected() {
        let billing = AgentBilling::with_endpoint("http://127.0.0.1:9".into());
        let now = std::time::UNIX_EPOCH + std::time::Duration::from_secs(1_000_000);
        let org = "11111111-1111-1111-1111-111111111111";
        let key = "aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa";
        let digest = request_digest("POST", "/v1/search", b"one");
        let Idempotency::Proceed { transaction_id } =
            billing.begin_idempotency(org, "/v1/search", "search_request", key, &digest, now)
        else {
            panic!("first request should proceed");
        };
        assert!(matches!(
            billing.begin_idempotency(org, "/v1/search", "search_request", key, &digest, now),
            Idempotency::Rejected(message) if message.contains("in progress")
        ));
        let other = request_digest("POST", "/v1/search", b"two");
        assert!(matches!(
            billing.begin_idempotency(org, "/v1/search", "search_request", key, &other, now),
            Idempotency::Rejected(message) if message.contains("does not match")
        ));
        billing
            .complete_idempotency(
                org,
                "/v1/search",
                key,
                &digest,
                StoredResponse {
                    status: 200,
                    content_type: Some("application/json".into()),
                    body: b"{}".to_vec(),
                },
            )
            .unwrap();
        match billing.begin_idempotency(org, "/v1/search", "search_request", key, &digest, now) {
            Idempotency::Replay(stored) => assert_eq!(stored.body, b"{}"),
            other => panic!("expected replay, got {other:?}"),
        }
        let other = request_digest("POST", "/v1/search", b"two");
        assert!(matches!(
            billing.begin_idempotency(org, "/v1/search", "search_request", key, &other, now),
            Idempotency::Rejected(message) if message.contains("does not match")
        ));
        let later = now + std::time::Duration::from_secs(24 * 60 * 60 + 5);
        match billing.begin_idempotency(org, "/v1/search", "search_request", key, &other, later) {
            Idempotency::Proceed {
                transaction_id: next,
            } => assert_ne!(transaction_id, next),
            other => panic!("expired key should proceed, got {other:?}"),
        }
    }

    #[test]
    fn a_server_error_is_not_a_stored_idempotent_response() {
        assert!(remember_idempotent_response(http::StatusCode::OK));
        assert!(remember_idempotent_response(
            http::StatusCode::UNPROCESSABLE_ENTITY
        ));
        assert!(!remember_idempotent_response(http::StatusCode::BAD_GATEWAY));
        assert!(!remember_idempotent_response(
            http::StatusCode::SERVICE_UNAVAILABLE
        ));
        assert!(!remember_idempotent_response(
            http::StatusCode::TOO_MANY_REQUESTS
        ));
    }

    #[test]
    fn a_query_string_changes_the_idempotency_digest() {
        assert_ne!(
            request_digest("POST", "/v1/search", b"{}"),
            request_digest("POST", "/v1/search?q=1", b"{}")
        );
    }

    #[tokio::test]
    async fn a_failed_usage_event_clears_the_subscription_cache() {
        use tokio::io::AsyncWriteExt;
        let seen = std::sync::Arc::new(tokio::sync::Mutex::new(0_u32));
        let count = std::sync::Arc::clone(&seen);
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        tokio::spawn(async move {
            loop {
                let Ok((mut socket, _)) = listener.accept().await else {
                    break;
                };
                let count = std::sync::Arc::clone(&count);
                tokio::spawn(async move {
                    let request = read_http(&mut socket).await;
                    let n = {
                        let mut guard = count.lock().await;
                        *guard += 1;
                        *guard
                    };
                    let fail = request.contains("transaction_id") && n == 2;
                    let payload = if fail {
                        br#"{"success":false}"#.to_vec()
                    } else {
                        br#"{"success":true,"data":{"duplicate":false}}"#.to_vec()
                    };
                    let status = if fail {
                        "500 Internal Server Error"
                    } else {
                        "200 OK"
                    };
                    let head = format!(
                        "HTTP/1.1 {status}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                        payload.len()
                    );
                    let _ = socket.write_all(head.as_bytes()).await;
                    let _ = socket.write_all(&payload).await;
                });
            }
        });
        let billing = AgentBilling::with_endpoint(format!("http://{address}"));
        let mut request = http::HeaderMap::new();
        request.insert(
            "x-aster-org",
            http::HeaderValue::from_static("11111111-1111-1111-1111-111111111111"),
        );
        let first = settle(
            Some(&billing),
            "search",
            "/v1/search",
            http::StatusCode::OK,
            &mut http::HeaderMap::new(),
            &request,
            None,
        )
        .await;
        assert!(first.is_err(), "{first:?}");
        settle(
            Some(&billing),
            "search",
            "/v1/search",
            http::StatusCode::OK,
            &mut http::HeaderMap::new(),
            &request,
            None,
        )
        .await
        .unwrap();
        assert!(
            *seen.lock().await >= 4,
            "a failed event must ensure the subscription again"
        );
    }

    #[test]
    fn an_unstored_charge_keeps_its_transaction_id_after_the_lease_drops() {
        let billing = AgentBilling::with_endpoint("http://127.0.0.1:9".into());
        let now = std::time::UNIX_EPOCH + std::time::Duration::from_secs(1_000_000);
        let org = "11111111-1111-1111-1111-111111111111";
        let key = "aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa";
        let digest = request_digest("POST", "/v1/parse", b"file");
        let Idempotency::Proceed { transaction_id } =
            billing.begin_idempotency(org, "/v1/parse", "parsed_page", key, &digest, now)
        else {
            panic!("first request should proceed");
        };
        {
            let mut lease = billing.lease(org, "/v1/parse", key);
            retain_idempotency_after_charge(&mut lease, false);
        }
        match billing.begin_idempotency(org, "/v1/parse", "parsed_page", key, &digest, now) {
            Idempotency::Proceed {
                transaction_id: again,
            } => assert_eq!(transaction_id, again),
            other => panic!("retry should keep the transaction id, got {other:?}"),
        }
    }

    #[test]
    fn a_failed_charge_reuses_the_same_transaction_id() {
        let billing = AgentBilling::with_endpoint("http://127.0.0.1:9".into());
        let now = std::time::UNIX_EPOCH + std::time::Duration::from_secs(1_000_000);
        let org = "11111111-1111-1111-1111-111111111111";
        let key = "aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa";
        let digest = request_digest("POST", "/v1/search", b"one");
        let Idempotency::Proceed { transaction_id } =
            billing.begin_idempotency(org, "/v1/search", "search_request", key, &digest, now)
        else {
            panic!("first request should proceed");
        };
        billing.mark_retryable(org, "/v1/search", key);
        match billing.begin_idempotency(org, "/v1/search", "search_request", key, &digest, now) {
            Idempotency::Proceed {
                transaction_id: again,
            } => assert_eq!(transaction_id, again),
            other => panic!("retry should keep the transaction id, got {other:?}"),
        }
    }

    #[test]
    fn dropping_an_idempotency_lease_releases_the_key() {
        let billing = AgentBilling::with_endpoint("http://127.0.0.1:9".into());
        let now = std::time::UNIX_EPOCH + std::time::Duration::from_secs(1_000_000);
        let org = "11111111-1111-1111-1111-111111111111";
        let key = "aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa";
        let digest = request_digest("POST", "/v1/search", b"one");
        let lease = billing.lease(org, "/v1/search", key);
        assert!(matches!(
            billing.begin_idempotency(org, "/v1/search", "search_request", key, &digest, now),
            Idempotency::Proceed { .. }
        ));
        drop(lease);
        assert!(matches!(
            billing.begin_idempotency(org, "/v1/search", "search_request", key, &digest, now),
            Idempotency::Proceed { .. }
        ));
    }

    #[test]
    fn search_and_parse_routes_require_billing_configuration() {
        assert!(!routes_need_agent_billing(["Path(/health)"]));
        assert!(routes_need_agent_billing(["Path(/v1/search)"]));
        assert!(require_billing_for_agent_routes(false, None).is_ok());
        match require_billing_for_agent_routes(true, None) {
            Err(message) => assert!(message.contains("ASTER_BILLING_URL"), "{message}"),
            Ok(_) => panic!("search and parse routes must refuse to start without billing"),
        }
        let billing = AgentBilling::with_endpoint("http://127.0.0.1:9".into());
        assert!(require_billing_for_agent_routes(true, Some(billing)).is_ok());
    }

    async fn accept_charges(seen: std::sync::Arc<tokio::sync::Mutex<Vec<String>>>) -> String {
        use tokio::io::AsyncWriteExt;
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        tokio::spawn(async move {
            loop {
                let Ok((mut socket, _)) = listener.accept().await else {
                    break;
                };
                let seen = std::sync::Arc::clone(&seen);
                tokio::spawn(async move {
                    let request = read_http(&mut socket).await;
                    let body = request.split("\r\n\r\n").nth(1).unwrap_or("").to_string();
                    seen.lock().await.push(body);
                    let payload = br#"{"success":true,"data":{"duplicate":true}}"#;
                    let head = format!(
                        "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                        payload.len()
                    );
                    let _ = socket.write_all(head.as_bytes()).await;
                    let _ = socket.write_all(payload).await;
                });
            }
        });
        format!("http://{address}")
    }

    async fn read_http(socket: &mut tokio::net::TcpStream) -> String {
        use tokio::io::AsyncReadExt;
        let mut buf = Vec::new();
        let mut tmp = [0u8; 1024];
        loop {
            let n = socket.read(&mut tmp).await.unwrap_or(0);
            if n == 0 {
                break;
            }
            buf.extend_from_slice(&tmp[..n]);
            let Some(end) = buf.windows(4).position(|window| window == b"\r\n\r\n") else {
                continue;
            };
            let header = String::from_utf8_lossy(&buf[..end]);
            let length = header.lines().find_map(|line| {
                let (name, value) = line.split_once(':')?;
                if name.eq_ignore_ascii_case("content-length") {
                    value.trim().parse::<usize>().ok()
                } else {
                    None
                }
            });
            if buf.len() >= end + 4 + length.unwrap_or(0) {
                break;
            }
        }
        String::from_utf8_lossy(&buf).into_owned()
    }
}
