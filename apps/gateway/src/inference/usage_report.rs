//! Report one signed-in inference call to a3s-api.
//!
//! The URL comes from `A3S_USAGE_INGEST_URL`. An empty URL records nothing and
//! logs that degrade once. The inference response is already complete; this
//! report is the API ledger input, not a second write into a3s-billing.
//! The API resolves the principal subject to an organization membership.

use super::token_reconcile::UsageAttribution;
use serde::Serialize;
use std::sync::{Arc, OnceLock};
use std::time::Duration;
use uuid::Uuid;

const ATTEMPTS: usize = 3;

#[derive(Debug, Serialize, PartialEq, Eq)]
pub(crate) struct UsageFactBody {
    transaction_id: String,
    subject_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    org_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    credential_id: Option<String>,
    model_id: String,
    endpoint: String,
    tokens: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    prompt_tokens: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    completion_tokens: Option<u64>,
    succeeded: bool,
    occurred_at: i64,
}

/// Resolve the principal to the member UUID the API ledger keys usage by.
///
/// A shared deployment token carries no member identity of its own. It is
/// attributed to the platform account named by `A3S_GATEWAY_SHARED_SUBJECT_ID`,
/// so the call is still metered even though that account is not charged.
fn platform_attribution(attribution: &UsageAttribution) -> Option<UsageAttribution> {
    let subject = attribution.subject.subject.trim();
    if Uuid::parse_str(subject).is_ok() {
        return Some(attribution.clone());
    }
    let mapped = std::env::var("A3S_GATEWAY_SHARED_SUBJECT_ID").ok()?;
    let mapped = mapped.trim();
    if Uuid::parse_str(mapped).is_err() {
        return None;
    }
    let mut resolved = attribution.clone();
    resolved.subject.subject = Arc::from(mapped);
    Some(resolved)
}

pub(crate) fn report_principal_usage(
    attribution: &UsageAttribution,
    total_tokens: u64,
    status: u16,
    prompt_tokens: Option<u64>,
    completion_tokens: Option<u64>,
) {
    let Some(attribution) = platform_attribution(attribution) else {
        static LOGGED: OnceLock<()> = OnceLock::new();
        if LOGGED.set(()).is_ok() {
            tracing::warn!(
                "usage ingest skipped: the principal subject is not a member id and \
                 A3S_GATEWAY_SHARED_SUBJECT_ID is unset"
            );
        }
        return;
    };
    let occurred_at = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_secs() as i64)
        .unwrap_or(0);
    let body = usage_fact_body(
        &attribution,
        total_tokens,
        status,
        occurred_at,
        prompt_tokens,
        completion_tokens,
    );
    let url = std::env::var("A3S_USAGE_INGEST_URL").unwrap_or_default();
    let token = std::env::var("A3S_GATEWAY_ADMIN_TOKEN").unwrap_or_default();
    submit_usage_fact(url.trim(), token.trim(), body);
}

pub(crate) fn usage_fact_body(
    attribution: &UsageAttribution,
    total_tokens: u64,
    status: u16,
    occurred_at: i64,
    prompt_tokens: Option<u64>,
    completion_tokens: Option<u64>,
) -> UsageFactBody {
    let org_id = attribution
        .subject
        .org
        .as_deref()
        .map(str::trim)
        .filter(|org| !org.is_empty())
        .map(str::to_owned);
    UsageFactBody {
        transaction_id: attribution.request_id.to_string(),
        subject_id: attribution.subject.subject.to_string(),
        org_id,
        credential_id: Some(attribution.subject.credential_id.to_string()),
        model_id: attribution.model.clone(),
        endpoint: attribution.endpoint.as_str().to_owned(),
        tokens: total_tokens,
        prompt_tokens,
        completion_tokens,
        succeeded: status < 400,
        occurred_at,
    }
}

pub(crate) fn submit_usage_fact(url: &str, token: &str, body: UsageFactBody) {
    if url.is_empty() {
        static LOGGED: OnceLock<()> = OnceLock::new();
        LOGGED.get_or_init(|| {
            tracing::error!(
                "usage ingest is disabled because A3S_USAGE_INGEST_URL is empty; signed-in calls are not reported"
            );
        });
        return;
    }
    if token.is_empty() {
        tracing::error!("usage ingest is configured without A3S_GATEWAY_ADMIN_TOKEN");
        return;
    }
    let Ok(handle) = tokio::runtime::Handle::try_current() else {
        tracing::error!("usage ingest has no async runtime");
        return;
    };
    let url = url.to_owned();
    let token = token.to_owned();
    handle.spawn(async move {
        if let Err(error) = post_usage_fact(&url, &token, &body).await {
            tracing::warn!(error = %error, "usage ingest failed");
        }
    });
}

fn retryable(status: u16) -> bool {
    status == 429 || status >= 500
}

async fn post_usage_fact(url: &str, token: &str, body: &UsageFactBody) -> Result<(), String> {
    let client = client().ok_or_else(|| "usage client is unavailable".to_owned())?;
    let mut delay = Duration::from_millis(200);
    for attempt in 1..=ATTEMPTS {
        match client.post(url).bearer_auth(token).json(body).send().await {
            Ok(response) => {
                let status = response.status().as_u16();
                if response.status().is_success() {
                    return Ok(());
                }
                let text = response.text().await.unwrap_or_default();
                if !retryable(status) || attempt == ATTEMPTS {
                    return Err(format!("status {status}: {}", truncate(&text)));
                }
            }
            Err(error) => {
                if attempt == ATTEMPTS {
                    return Err(error.to_string());
                }
            }
        }
        tokio::time::sleep(delay).await;
        delay = delay.saturating_mul(2);
    }
    Err("usage ingest failed".into())
}

fn truncate(text: &str) -> String {
    text.chars().take(180).collect()
}

fn client() -> Option<&'static reqwest::Client> {
    static CLIENT: OnceLock<Option<reqwest::Client>> = OnceLock::new();
    CLIENT
        .get_or_init(|| {
            reqwest::Client::builder()
                .timeout(Duration::from_secs(5))
                .connect_timeout(Duration::from_secs(2))
                .redirect(reqwest::redirect::Policy::none())
                .build()
                .map_err(|error| {
                    tracing::error!(error = %error, "Could not build usage ingest client");
                })
                .ok()
        })
        .as_ref()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::InferenceEndpoint;
    use crate::inference::PrincipalSubject;
    use std::sync::Arc;

    fn attribution(org: Option<&str>) -> UsageAttribution {
        UsageAttribution {
            subject: PrincipalSubject {
                credential_id: uuid::Uuid::nil(),
                subject: Arc::from("11111111-1111-1111-1111-111111111111"),
                org: org.map(Arc::from),
                allowed_models: None,
                rpm_limit: None,
            },
            model: "boyue/demo".into(),
            endpoint: InferenceEndpoint::ChatCompletions,
            request_id: uuid::Uuid::nil(),
        }
    }

    #[test]
    fn an_empty_org_is_omitted() {
        let body = usage_fact_body(&attribution(Some("  ")), 12, 200, 1_700_000_000, None, None);
        let json = serde_json::to_value(&body).unwrap();
        assert!(json.get("org_id").is_none());
        assert_eq!(json["subject_id"], "11111111-1111-1111-1111-111111111111");
        assert_eq!(json["endpoint"], "chat-completions");
        assert_eq!(json["tokens"], 12);
        assert_eq!(json["succeeded"], true);
        assert_eq!(json["occurred_at"], 1_700_000_000);
    }

    #[test]
    fn a_client_org_is_only_a_hint() {
        let body = usage_fact_body(
            &attribution(Some("org-1")),
            0,
            500,
            1_700_000_000,
            None,
            None,
        );
        assert_eq!(body.org_id.as_deref(), Some("org-1"));
        assert!(!body.succeeded);
    }

    #[test]
    fn only_429_and_5xx_are_retried() {
        assert!(retryable(429));
        assert!(retryable(500));
        assert!(retryable(503));
        assert!(!retryable(200));
        assert!(!retryable(400));
        assert!(!retryable(401));
        assert!(!retryable(404));
    }

    #[test]
    fn the_platform_account_receives_shared_usage() {
        let platform = "22222222-2222-2222-2222-222222222222";

        // A member id passes through untouched.
        let member = attribution(None);
        let resolved = platform_attribution(&member).expect("member id resolves");
        assert_eq!(
            &*resolved.subject.subject,
            "11111111-1111-1111-1111-111111111111"
        );

        // A shared token is attributed to the platform account when configured.
        let mut shared = attribution(None);
        shared.subject.subject = std::sync::Arc::from("shared");
        std::env::set_var("A3S_GATEWAY_SHARED_SUBJECT_ID", platform);
        let resolved = platform_attribution(&shared).expect("shared subject resolves");
        assert_eq!(&*resolved.subject.subject, platform);

        // Without a configured account the fact is dropped rather than sent
        // with a subject the ledger cannot key on.
        std::env::remove_var("A3S_GATEWAY_SHARED_SUBJECT_ID");
        assert!(platform_attribution(&shared).is_none());
    }

    #[test]
    fn an_empty_url_does_not_need_a_runtime() {
        submit_usage_fact(
            "",
            "token",
            usage_fact_body(&attribution(None), 1, 200, 1, None, None),
        );
    }
}
