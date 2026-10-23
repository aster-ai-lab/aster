//! Signed-in user verification for principal inference routes.
//!
//! The caller credential (session token or personal API key) is forwarded to
//! the configured OS auth `verify_url`. A 2xx answer carrying the subject
//! header authenticates the caller; the outcome is cached by credential
//! digest so steady-state requests do not round-trip to auth.

use super::access_error::InferenceAccessError;
use crate::config::InferencePrincipalConfig;
use http::header::AUTHORIZATION;
use http::HeaderMap;
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::sync::{Arc, Mutex, OnceLock, PoisonError};
use std::time::{Duration, Instant};
use uuid::Uuid;

const MAX_PRINCIPAL_TOKEN_BYTES: usize = 4_096;
const MAX_SUBJECT_BYTES: usize = 255;
const MAX_CACHED_PRINCIPALS: usize = 65_536;
const VERIFY_TIMEOUT: Duration = Duration::from_secs(5);
const VERIFY_CONNECT_TIMEOUT: Duration = Duration::from_secs(2);
const X_API_KEY: &str = "x-api-key";

/// Namespace for deterministic per-route subject credential IDs.
const PRINCIPAL_NAMESPACE: Uuid = Uuid::from_u128(0x5f0c_2d1e_8a47_4b8e_9c3a_a3c0_0517_0001);

/// One verified signed-in caller.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct PrincipalSubject {
    pub(crate) credential_id: Uuid,
    pub(crate) subject: Arc<str>,
    pub(crate) org: Option<Arc<str>>,
}

#[derive(Clone)]
struct CachedPrincipal {
    route_id: Uuid,
    subject: PrincipalSubject,
    expires_at: Instant,
}

pub(super) struct PrincipalVerifier {
    verified: Arc<Mutex<HashMap<[u8; 32], CachedPrincipal>>>,
    subjects: Arc<Mutex<HashMap<Uuid, PrincipalSubject>>>,
}

impl PrincipalVerifier {
    /// Share the cache across snapshot reloads; each entry stays bound to the
    /// route it was verified for and to its own TTL.
    pub(super) fn new(previous: Option<&Self>) -> Self {
        match previous {
            Some(previous) => Self {
                verified: previous.verified.clone(),
                subjects: previous.subjects.clone(),
            },
            None => Self {
                verified: Arc::new(Mutex::new(HashMap::new())),
                subjects: Arc::new(Mutex::new(HashMap::new())),
            },
        }
    }

    pub(super) fn subject(&self, credential_id: Uuid) -> Option<PrincipalSubject> {
        self.subjects
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .get(&credential_id)
            .cloned()
    }

    fn remember_subject(&self, subject: &PrincipalSubject) {
        let mut subjects = self.subjects.lock().unwrap_or_else(PoisonError::into_inner);
        if subjects.len() >= MAX_CACHED_PRINCIPALS && !subjects.contains_key(&subject.credential_id)
        {
            subjects.clear();
        }
        subjects.insert(subject.credential_id, subject.clone());
    }

    pub(super) async fn verify(
        &self,
        route_id: Uuid,
        principal: &InferencePrincipalConfig,
        headers: &HeaderMap,
    ) -> Result<PrincipalSubject, InferenceAccessError> {
        // A3S_GATEWAY_SHARED_TOKEN skips verify_url. Any other presented credential is checked. No credential is unauthorized.
        let token = principal_token(headers)?;
        if let Some(subject) = shared_subject(route_id, token) {
            self.remember_subject(&subject);
            return Ok(subject);
        }
        let digest: [u8; 32] = Sha256::digest(token.as_bytes()).into();
        let now = Instant::now();
        {
            let cache = self.verified.lock().unwrap_or_else(PoisonError::into_inner);
            if let Some(cached) = cache.get(&digest) {
                if cached.route_id == route_id && cached.expires_at > now {
                    return Ok(cached.subject.clone());
                }
            }
        }

        let (subject, org) = call_verify(principal, token).await?;
        let subject = PrincipalSubject {
            credential_id: Uuid::new_v5(
                &PRINCIPAL_NAMESPACE,
                format!("{route_id}/{subject}").as_bytes(),
            ),
            subject: Arc::from(subject),
            org: org.map(Arc::from),
        };
        let expires_at = Instant::now() + Duration::from_secs(principal.cache_ttl_secs);
        {
            let mut cache = self.verified.lock().unwrap_or_else(PoisonError::into_inner);
            if cache.len() >= MAX_CACHED_PRINCIPALS {
                let now = Instant::now();
                cache.retain(|_, cached| cached.expires_at > now);
                if cache.len() >= MAX_CACHED_PRINCIPALS {
                    cache.clear();
                }
            }
            cache.insert(
                digest,
                CachedPrincipal {
                    route_id,
                    subject: subject.clone(),
                    expires_at,
                },
            );
        }
        {
            let mut subjects = self.subjects.lock().unwrap_or_else(PoisonError::into_inner);
            if subjects.len() >= MAX_CACHED_PRINCIPALS {
                subjects.clear();
            }
            subjects.insert(subject.credential_id, subject.clone());
        }
        Ok(subject)
    }
}

/// Accept `Authorization: Bearer <token>` (OpenAI SDKs) or `x-api-key`
/// (Anthropic SDKs). Exactly one credential must be present.
fn principal_token(headers: &HeaderMap) -> Result<&str, InferenceAccessError> {
    let bearer = single_header(headers, AUTHORIZATION.as_str())?
        .map(|value| {
            let (scheme, token) = value
                .split_once(' ')
                .ok_or(InferenceAccessError::Unauthorized)?;
            if !scheme.eq_ignore_ascii_case("bearer") {
                return Err(InferenceAccessError::Unauthorized);
            }
            Ok(token.trim())
        })
        .transpose()?;
    let api_key = single_header(headers, X_API_KEY)?.map(str::trim);
    let token = match (bearer, api_key) {
        (Some(bearer), Some(api_key)) if bearer != api_key => {
            return Err(InferenceAccessError::Unauthorized)
        }
        (Some(token), _) | (None, Some(token)) => token,
        (None, None) => return Err(InferenceAccessError::Unauthorized),
    };
    if token.is_empty()
        || token.len() > MAX_PRINCIPAL_TOKEN_BYTES
        || !token.bytes().all(|byte| byte.is_ascii_graphic())
    {
        return Err(InferenceAccessError::Unauthorized);
    }
    Ok(token)
}

/// Whether the caller presented the deployment-wide shared token.
///
/// Managed inference admits that credential without a `verify_url` round
/// trip. Forward-auth routes reuse the same rule so one deployment keeps one
/// key for every entry point.
pub(crate) fn shared_token_admits(headers: &HeaderMap) -> bool {
    let Ok(token) = principal_token(headers) else {
        return false;
    };
    let Ok(expected) = std::env::var("A3S_GATEWAY_SHARED_TOKEN") else {
        return false;
    };
    let expected = expected.trim();
    !expected.is_empty() && tokens_match(token, expected)
}

fn shared_subject(route_id: Uuid, token: &str) -> Option<PrincipalSubject> {
    let Ok(expected) = std::env::var("A3S_GATEWAY_SHARED_TOKEN") else {
        return None;
    };
    let expected = expected.trim();
    if expected.is_empty() || !tokens_match(token, expected) {
        return None;
    }
    let subject = "shared";
    Some(PrincipalSubject {
        credential_id: Uuid::new_v5(
            &PRINCIPAL_NAMESPACE,
            format!("{route_id}/{subject}").as_bytes(),
        ),
        subject: Arc::from(subject),
        org: None,
    })
}

fn tokens_match(presented: &str, expected: &str) -> bool {
    let presented = presented.as_bytes();
    let expected = expected.as_bytes();
    if presented.len() != expected.len() {
        return false;
    }
    let mut diff = 0u8;
    for (left, right) in presented.iter().zip(expected.iter()) {
        diff |= left ^ right;
    }
    diff == 0
}

fn single_header<'a>(
    headers: &'a HeaderMap,
    name: &str,
) -> Result<Option<&'a str>, InferenceAccessError> {
    let mut values = headers.get_all(name).iter();
    let Some(value) = values.next() else {
        return Ok(None);
    };
    if values.next().is_some() {
        return Err(InferenceAccessError::Unauthorized);
    }
    value
        .to_str()
        .map(Some)
        .map_err(|_| InferenceAccessError::Unauthorized)
}

fn client() -> Option<&'static reqwest::Client> {
    static CLIENT: OnceLock<Option<reqwest::Client>> = OnceLock::new();
    CLIENT
        .get_or_init(|| {
            reqwest::Client::builder()
                .timeout(VERIFY_TIMEOUT)
                .connect_timeout(VERIFY_CONNECT_TIMEOUT)
                .redirect(reqwest::redirect::Policy::none())
                .build()
                .map_err(|error| {
                    tracing::error!(error = %error, "Could not build principal verify client");
                })
                .ok()
        })
        .as_ref()
}

async fn call_verify(
    principal: &InferencePrincipalConfig,
    token: &str,
) -> Result<(String, Option<String>), InferenceAccessError> {
    let client = client().ok_or(InferenceAccessError::Unavailable)?;
    let response = client
        .get(&principal.verify_url)
        .bearer_auth(token)
        .send()
        .await
        .map_err(|error| {
            tracing::warn!(error = %error, "Principal verify request failed");
            InferenceAccessError::Unavailable
        })?;
    let status = response.status();
    if status == reqwest::StatusCode::UNAUTHORIZED || status == reqwest::StatusCode::FORBIDDEN {
        return Err(InferenceAccessError::Unauthorized);
    }
    if !status.is_success() {
        tracing::warn!(status = %status, "Principal verify answered with an unexpected status");
        return Err(InferenceAccessError::Unavailable);
    }
    let header = |name: &str| {
        response
            .headers()
            .get(name)
            .and_then(|value| value.to_str().ok())
            .map(str::trim)
            .filter(|value| !value.is_empty() && value.len() <= MAX_SUBJECT_BYTES)
            .map(str::to_owned)
    };
    let subject = header(&principal.subject_header).ok_or_else(|| {
        tracing::warn!("Principal verify answered 2xx without a subject header");
        InferenceAccessError::Unauthorized
    })?;
    let org = principal.org_header.as_deref().and_then(header);
    Ok((subject, org))
}

#[cfg(test)]
mod tests {
    use super::*;
    use http::HeaderValue;

    fn headers(pairs: &[(&'static str, &'static str)]) -> HeaderMap {
        let mut headers = HeaderMap::new();
        for (name, value) in pairs {
            headers.append(*name, HeaderValue::from_static(value));
        }
        headers
    }

    #[test]
    fn token_accepts_bearer_or_x_api_key() {
        assert_eq!(
            principal_token(&headers(&[("authorization", "Bearer a3s_sk_one")])).unwrap(),
            "a3s_sk_one"
        );
        assert_eq!(
            principal_token(&headers(&[("x-api-key", "a3s_sk_two")])).unwrap(),
            "a3s_sk_two"
        );
        assert_eq!(
            principal_token(&headers(&[
                ("authorization", "Bearer a3s_sk_same"),
                ("x-api-key", "a3s_sk_same"),
            ]))
            .unwrap(),
            "a3s_sk_same"
        );
    }

    #[test]
    fn token_rejects_missing_conflicting_or_malformed_credentials() {
        for case in [
            headers(&[]),
            headers(&[("authorization", "Basic abc")]),
            headers(&[("authorization", "Bearer ")]),
            headers(&[("authorization", "Bearer a"), ("x-api-key", "b")]),
            headers(&[("x-api-key", "a"), ("x-api-key", "a")]),
        ] {
            assert!(matches!(
                principal_token(&case),
                Err(InferenceAccessError::Unauthorized)
            ));
        }
    }

    #[test]
    fn missing_credential_is_unauthorized_without_calling_verify() {
        let verifier = PrincipalVerifier::new(None);
        let route_id = Uuid::nil();
        let principal = crate::config::InferencePrincipalConfig {
            verify_url: "http://127.0.0.1:9/auth/verify".into(),
            subject_header: "x-a3s-subject".into(),
            org_header: None,
            cache_ttl_secs: 60,
            models: vec!["*".into()],
            endpoints: Vec::new(),
            limits: crate::config::InferenceLimitsConfig {
                max_spend_micros: None,
                spend_window: None,
                spend_baseline_micros: None,
                baseline_epoch: None,
                max_concurrent_requests: 4,
                requests_per_minute: 60,
                request_burst: 10,
                tokens_per_minute: 100_000,
            },
        };
        let result = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap()
            .block_on(verifier.verify(route_id, &principal, &HeaderMap::new()));
        assert!(matches!(result, Err(InferenceAccessError::Unauthorized)));
    }

    #[test]
    fn shared_token_skips_verify() {
        let token = "a3s_gw_testtoken";
        std::env::set_var("A3S_GATEWAY_SHARED_TOKEN", token);
        let verifier = PrincipalVerifier::new(None);
        let principal = crate::config::InferencePrincipalConfig {
            verify_url: "http://127.0.0.1:9/auth/verify".into(),
            subject_header: "x-a3s-subject".into(),
            org_header: None,
            cache_ttl_secs: 60,
            models: vec!["*".into()],
            endpoints: Vec::new(),
            limits: crate::config::InferenceLimitsConfig {
                max_spend_micros: None,
                spend_window: None,
                spend_baseline_micros: None,
                baseline_epoch: None,
                max_concurrent_requests: 4,
                requests_per_minute: 60,
                request_burst: 10,
                tokens_per_minute: 100_000,
            },
        };
        let headers = headers(&[("authorization", "Bearer a3s_gw_testtoken")]);
        let subject = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap()
            .block_on(verifier.verify(Uuid::nil(), &principal, &headers))
            .unwrap();
        std::env::remove_var("A3S_GATEWAY_SHARED_TOKEN");
        assert_eq!(&*subject.subject, "shared");
    }
}
