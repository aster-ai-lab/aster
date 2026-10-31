//! Prompt-free managed inference lifecycle evidence.
//!
//! These events are Gateway's durable spool payload bytes. After Cloud unwraps
//! `aster.gateway.usage-batch.v1`, Inference validates each payload against
//! `aster.gateway.usage-lifecycle.v2` (and still accepts v1 records replayed
//! from older spools) before ledger insertion. Keep encode shape aligned with
//! that contract; do not embed prompts or credential secrets.

use super::{UsageReservation, UsageSpool, UsageSpoolError};
use crate::inference::{
    AuthenticatedInference, InferenceAttemptIdentity, InferenceRequestIdentity,
    LifecycleTokenBreakdown, ObservedTokenUsage,
};
use crate::response_body::ResponseBody;
use bytes::Bytes;
use chrono::{DateTime, Utc};
use http::{Response, StatusCode};
use hyper::body::{Body, Frame, SizeHint};
use serde::Serialize;
use std::pin::Pin;
use std::sync::Arc;
use std::task::{Context, Poll};
use std::time::Instant;
use uuid::Uuid;

const LIFECYCLE_SCHEMA: &str = "aster.gateway.usage-lifecycle.v2";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum UsageTerminalOutcome {
    Succeeded,
    Failed,
    Fallback,
    Cancelled,
    Disconnected,
}

#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "snake_case")]
enum MeasurementCompleteness {
    Unknown,
    UpstreamUsage,
}

/// HR5: the prompt-free task-routing facts riding the lifecycle
/// events — mode, chosen group, decision reason, and whether a frozen
/// turn was reused. Absent on unrouted requests (their wire stays
/// byte-identical).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) struct RoutingEvidence {
    pub(crate) mode: String,
    pub(crate) group: String,
    pub(crate) reason: String,
    pub(crate) frozen: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "snake_case")]
struct RequestEvidence {
    request_id: Uuid,
    correlation_id: String,
    environment_id: Uuid,
    credential_id: Uuid,
    credential_generation: u64,
    route_id: Uuid,
    route_policy_revision: u64,
    endpoint: crate::config::InferenceEndpoint,
    model_alias: String,
    model_id: Uuid,
    #[serde(skip_serializing_if = "Option::is_none")]
    routing: Option<RoutingEvidence>,
}

impl RequestEvidence {
    fn new(
        identity: &InferenceRequestIdentity,
        authenticated: AuthenticatedInference,
        model_alias: &str,
        routing: Option<RoutingEvidence>,
    ) -> Result<Self, UsageSpoolError> {
        let model_id = identity.model_id().ok_or_else(|| UsageSpoolError::Encode {
            reason: "managed inference request has no resolved model identity".to_string(),
        })?;
        Ok(Self {
            request_id: identity.request_id(),
            correlation_id: identity.correlation_id().to_string(),
            environment_id: authenticated.environment_id(),
            credential_id: authenticated.credential_id(),
            credential_generation: authenticated.credential_generation(),
            route_id: identity.route_id(),
            route_policy_revision: identity.route_policy_revision(),
            endpoint: identity.endpoint(),
            model_alias: model_alias.to_string(),
            model_id,
            routing,
        })
    }
}

#[derive(Debug, Clone, Serialize)]
struct AttemptEvidence {
    attempt_id: Uuid,
    target_id: Uuid,
}

impl From<&InferenceAttemptIdentity> for AttemptEvidence {
    fn from(identity: &InferenceAttemptIdentity) -> Self {
        Self {
            attempt_id: identity.attempt_id(),
            target_id: identity.target_id(),
        }
    }
}

#[derive(Debug, Serialize)]
struct LifecycleEvent<'a> {
    schema: &'static str,
    kind: LifecycleEventKind,
    occurred_at: DateTime<Utc>,
    request: &'a RequestEvidence,
    #[serde(skip_serializing_if = "Option::is_none")]
    attempt: Option<&'a AttemptEvidence>,
    #[serde(skip_serializing_if = "Option::is_none")]
    outcome: Option<UsageTerminalOutcome>,
    #[serde(skip_serializing_if = "Option::is_none")]
    http_status: Option<u16>,
    #[serde(skip_serializing_if = "Option::is_none")]
    duration_ms: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    measurement_completeness: Option<MeasurementCompleteness>,
    #[serde(skip_serializing_if = "Option::is_none")]
    total_tokens: Option<u64>,
    /// Token split observed from the upstream usage; v2 payloads carry it
    /// whenever any lane was observed, even when the total is unresolvable.
    #[serde(skip_serializing_if = "Option::is_none")]
    tokens: Option<&'a LifecycleTokenBreakdown>,
}

#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "snake_case")]
enum LifecycleEventKind {
    RequestStarted,
    AttemptStarted,
    AttemptTerminal,
    RequestTerminal,
}

impl<'a> LifecycleEvent<'a> {
    fn request_started(request: &'a RequestEvidence) -> Self {
        Self {
            schema: LIFECYCLE_SCHEMA,
            kind: LifecycleEventKind::RequestStarted,
            occurred_at: Utc::now(),
            request,
            attempt: None,
            outcome: None,
            http_status: None,
            duration_ms: None,
            measurement_completeness: None,
            total_tokens: None,
            tokens: None,
        }
    }

    fn attempt_started(request: &'a RequestEvidence, attempt: &'a AttemptEvidence) -> Self {
        Self {
            schema: LIFECYCLE_SCHEMA,
            kind: LifecycleEventKind::AttemptStarted,
            occurred_at: Utc::now(),
            request,
            attempt: Some(attempt),
            outcome: None,
            http_status: None,
            duration_ms: None,
            measurement_completeness: None,
            total_tokens: None,
            tokens: None,
        }
    }

    fn terminal(
        kind: LifecycleEventKind,
        request: &'a RequestEvidence,
        attempt: Option<&'a AttemptEvidence>,
        terminal: TerminalEvidence<'a>,
    ) -> Self {
        let measurement_completeness =
            if terminal.total_tokens.is_some() || terminal.tokens.is_some() {
                MeasurementCompleteness::UpstreamUsage
            } else {
                MeasurementCompleteness::Unknown
            };
        Self {
            schema: LIFECYCLE_SCHEMA,
            kind,
            occurred_at: Utc::now(),
            request,
            attempt,
            outcome: Some(terminal.outcome),
            http_status: terminal.http_status,
            duration_ms: Some(terminal.duration_ms),
            measurement_completeness: Some(measurement_completeness),
            total_tokens: terminal.total_tokens,
            tokens: terminal.tokens,
        }
    }
}

/// Terminal facts shared by attempt and request terminal events.
struct TerminalEvidence<'a> {
    outcome: UsageTerminalOutcome,
    http_status: Option<u16>,
    duration_ms: u64,
    total_tokens: Option<u64>,
    tokens: Option<&'a LifecycleTokenBreakdown>,
}

#[derive(Debug)]
struct AttemptLifecycle {
    evidence: AttemptEvidence,
    started_at: Instant,
    terminal_event_id: Uuid,
    terminal_reservation: UsageReservation,
}

/// Durable request state retained from the first dispatch until the response
/// body completes, fails, disconnects, or is cancelled.
#[derive(Debug)]
pub(crate) struct UsageRequestLifecycle {
    spool: Arc<UsageSpool>,
    request: RequestEvidence,
    started_at: Instant,
    terminal_event_id: Uuid,
    terminal_reservation: Option<UsageReservation>,
    attempt: Option<AttemptLifecycle>,
    /// Shared with response-body token observation.
    observed_tokens: Arc<ObservedTokenUsage>,
}

impl UsageRequestLifecycle {
    pub(crate) async fn begin(
        spool: Arc<UsageSpool>,
        identity: &InferenceRequestIdentity,
        authenticated: AuthenticatedInference,
        model_alias: &str,
        routing: Option<RoutingEvidence>,
    ) -> Result<Self, UsageSpoolError> {
        let request = RequestEvidence::new(identity, authenticated, model_alias, routing)?;
        let payload = encode(&LifecycleEvent::request_started(&request))?;
        let (_, terminal_reservation) = spool
            .append_reserving_terminal(Uuid::new_v4(), &payload)
            .await?;
        Ok(Self {
            spool,
            request,
            started_at: Instant::now(),
            terminal_event_id: Uuid::new_v4(),
            terminal_reservation: Some(terminal_reservation),
            attempt: None,
            observed_tokens: Arc::new(ObservedTokenUsage::new()),
        })
    }

    /// Handle shared with token-budget observation on the response body.
    pub(crate) fn observed_tokens_handle(&self) -> Arc<ObservedTokenUsage> {
        self.observed_tokens.clone()
    }

    fn observed_usage(&self) -> (Option<u64>, Option<LifecycleTokenBreakdown>) {
        (
            self.observed_tokens.total(),
            self.observed_tokens.breakdown(),
        )
    }

    pub(crate) async fn begin_attempt(
        &mut self,
        identity: &InferenceAttemptIdentity,
    ) -> Result<(), UsageSpoolError> {
        if self.attempt.is_some() {
            return Err(UsageSpoolError::Encode {
                reason: "managed inference attempt started before its predecessor terminated"
                    .to_string(),
            });
        }
        let evidence = AttemptEvidence::from(identity);
        let payload = encode(&LifecycleEvent::attempt_started(&self.request, &evidence))?;
        let (_, terminal_reservation) = self
            .spool
            .append_reserving_terminal(Uuid::new_v4(), &payload)
            .await?;
        self.attempt = Some(AttemptLifecycle {
            evidence,
            started_at: Instant::now(),
            terminal_event_id: Uuid::new_v4(),
            terminal_reservation,
        });
        Ok(())
    }

    /// Persist an attempt terminal before another fallback attempt is allowed
    /// to dispatch. Waiting here preserves lifecycle ordering across fallback.
    pub(crate) async fn finish_attempt(
        &mut self,
        outcome: UsageTerminalOutcome,
        http_status: Option<u16>,
    ) -> Result<(), UsageSpoolError> {
        let Some(attempt) = self.attempt.take() else {
            return Ok(());
        };
        let payload = encode(&LifecycleEvent::terminal(
            LifecycleEventKind::AttemptTerminal,
            &self.request,
            Some(&attempt.evidence),
            TerminalEvidence {
                outcome,
                http_status,
                duration_ms: elapsed_ms(attempt.started_at),
                total_tokens: None,
                tokens: None,
            },
        ))?;
        attempt
            .terminal_reservation
            .commit(attempt.terminal_event_id, payload)?
            .wait()
            .await?;
        Ok(())
    }

    fn finish_background(&mut self, outcome: UsageTerminalOutcome, http_status: Option<u16>) {
        let (total_tokens, tokens) = self.observed_usage();
        if let Some(attempt) = self.attempt.take() {
            let event = LifecycleEvent::terminal(
                LifecycleEventKind::AttemptTerminal,
                &self.request,
                Some(&attempt.evidence),
                TerminalEvidence {
                    outcome,
                    http_status,
                    duration_ms: elapsed_ms(attempt.started_at),
                    total_tokens,
                    tokens: tokens.as_ref(),
                },
            );
            enqueue_terminal(
                attempt.terminal_reservation,
                attempt.terminal_event_id,
                &event,
            );
        }
        if let Some(reservation) = self.terminal_reservation.take() {
            let event = LifecycleEvent::terminal(
                LifecycleEventKind::RequestTerminal,
                &self.request,
                None,
                TerminalEvidence {
                    outcome,
                    http_status,
                    duration_ms: elapsed_ms(self.started_at),
                    total_tokens,
                    tokens: tokens.as_ref(),
                },
            );
            enqueue_terminal(reservation, self.terminal_event_id, &event);
        }
    }
}

impl Drop for UsageRequestLifecycle {
    fn drop(&mut self) {
        self.finish_background(UsageTerminalOutcome::Cancelled, None);
    }
}

/// Attach terminal usage delivery to the actual response-body lifetime.
pub(crate) fn track_usage_response(
    response: Response<ResponseBody>,
    lifecycle: Option<UsageRequestLifecycle>,
) -> Response<ResponseBody> {
    let Some(mut lifecycle) = lifecycle else {
        return response;
    };
    let status = response.status();
    if response.body().is_end_stream() {
        lifecycle.finish_background(outcome_for_status(status), Some(status.as_u16()));
        return response;
    }
    let (parts, body) = response.into_parts();
    let body = ResponseBody::boxed(UsageTrackedBody {
        inner: Box::pin(body),
        lifecycle: Some(lifecycle),
        status,
    });
    Response::from_parts(parts, body)
}

struct UsageTrackedBody {
    inner: Pin<Box<ResponseBody>>,
    lifecycle: Option<UsageRequestLifecycle>,
    status: StatusCode,
}

impl UsageTrackedBody {
    fn finish(&mut self, outcome: UsageTerminalOutcome) {
        if let Some(mut lifecycle) = self.lifecycle.take() {
            lifecycle.finish_background(outcome, Some(self.status.as_u16()));
        }
    }

    fn status_outcome(&self) -> UsageTerminalOutcome {
        outcome_for_status(self.status)
    }
}

impl Body for UsageTrackedBody {
    type Data = Bytes;
    type Error = std::io::Error;

    fn poll_frame(
        self: Pin<&mut Self>,
        context: &mut Context<'_>,
    ) -> Poll<Option<Result<Frame<Self::Data>, Self::Error>>> {
        let this = self.get_mut();
        let result = this.inner.as_mut().poll_frame(context);
        let inner_ended = this.inner.is_end_stream();
        match &result {
            Poll::Ready(Some(Err(_))) => this.finish(UsageTerminalOutcome::Failed),
            Poll::Ready(Some(Ok(_))) if inner_ended => this.finish(this.status_outcome()),
            Poll::Ready(None) => this.finish(this.status_outcome()),
            Poll::Pending | Poll::Ready(Some(Ok(_))) => {}
        }
        result
    }

    fn is_end_stream(&self) -> bool {
        self.lifecycle.is_none() && self.inner.is_end_stream()
    }

    fn size_hint(&self) -> SizeHint {
        self.inner.size_hint()
    }
}

impl Drop for UsageTrackedBody {
    fn drop(&mut self) {
        self.finish(UsageTerminalOutcome::Disconnected);
    }
}

fn enqueue_terminal(reservation: UsageReservation, event_id: Uuid, event: &LifecycleEvent<'_>) {
    match encode(event).and_then(|payload| reservation.commit(event_id, payload)) {
        Ok(receipt) => {
            if let Ok(runtime) = tokio::runtime::Handle::try_current() {
                runtime.spawn(async move {
                    if let Err(error) = receipt.wait().await {
                        tracing::error!(error = %error, "Durable usage terminal append failed");
                    }
                });
            }
        }
        Err(error) => {
            tracing::error!(error = %error, "Durable usage terminal enqueue failed");
        }
    }
}

fn encode(event: &LifecycleEvent<'_>) -> Result<Vec<u8>, UsageSpoolError> {
    serde_json::to_vec(event).map_err(|error| UsageSpoolError::Encode {
        reason: error.to_string(),
    })
}

fn elapsed_ms(started_at: Instant) -> u64 {
    u64::try_from(started_at.elapsed().as_millis()).unwrap_or(u64::MAX)
}

fn outcome_for_status(status: StatusCode) -> UsageTerminalOutcome {
    if status.is_success() || status.is_redirection() {
        UsageTerminalOutcome::Succeeded
    } else {
        UsageTerminalOutcome::Failed
    }
}

#[cfg(test)]
#[path = "lifecycle_contract_tests.rs"]
mod contract_tests;

#[cfg(test)]
mod tests {

    #[test]
    fn hr5_routed_requests_carry_prompt_free_routing_facts() {
        let request = RequestEvidence {
            request_id: Uuid::from_u128(1),
            correlation_id: "corr".into(),
            environment_id: Uuid::from_u128(2),
            credential_id: Uuid::from_u128(3),
            credential_generation: 1,
            route_id: Uuid::from_u128(4),
            route_policy_revision: 1,
            endpoint: InferenceEndpoint::ChatCompletions,
            model_alias: "alias".into(),
            model_id: Uuid::from_u128(5),
            routing: Some(RoutingEvidence {
                mode: "smart_saving".into(),
                group: "economy".into(),
                reason: "simple_probability".into(),
                frozen: false,
            }),
        };
        let encoded = encode(&LifecycleEvent::request_started(&request)).unwrap();
        let value: serde_json::Value = serde_json::from_slice(&encoded).unwrap();
        assert_eq!(value["request"]["routing"]["mode"], "smart_saving");
        assert_eq!(value["request"]["routing"]["group"], "economy");
        assert_eq!(value["request"]["routing"]["reason"], "simple_probability");
        assert_eq!(value["request"]["routing"]["frozen"], false);
        // Prompt-free: no content field rides the facts.
        let routing = value["request"]["routing"].as_object().unwrap();
        assert_eq!(routing.len(), 4);
    }

    use super::*;
    use crate::config::InferenceEndpoint;
    use uuid::Uuid;

    #[test]
    fn request_started_payload_matches_cloud_lifecycle_schema() {
        let request = RequestEvidence {
            request_id: Uuid::from_u128(1),
            correlation_id: "corr".into(),
            environment_id: Uuid::from_u128(2),
            credential_id: Uuid::from_u128(3),
            credential_generation: 1,
            route_id: Uuid::from_u128(4),
            route_policy_revision: 1,
            endpoint: InferenceEndpoint::ChatCompletions,
            model_alias: "alias".into(),
            model_id: Uuid::from_u128(5),
            routing: None,
        };
        let encoded = encode(&LifecycleEvent::request_started(&request)).unwrap();
        let value: serde_json::Value = serde_json::from_slice(&encoded).unwrap();
        assert_eq!(value["schema"], LIFECYCLE_SCHEMA);
        assert_eq!(value["kind"], "request_started");
        assert_eq!(value["request"]["endpoint"], "chat-completions");
        assert!(value.get("prompt").is_none());
        // HR5: an unrouted request carries NO routing block — the wire
        // stays byte-identical for every pre-HR5 request.
        assert!(value["request"].get("routing").is_none());
        assert!(value.get("messages").is_none());
    }
}
