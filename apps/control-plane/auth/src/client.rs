//! HTTP client for a remote `aster-auth` process.
//!
//! Extension-plane callers (aster-api) must use this client instead of opening
//! an AuthStore in-process. Domain types stay shared; storage stays remote.

use async_trait::async_trait;
use serde::Deserialize;
use serde_json::{json, Value};
use uuid::Uuid;

use crate::contexts::proof::domain::{
    Attestation, AuthCode, AuthError, IntrospectOutcome, IssuedChallenge, IssuedSession,
    LoginCommand, LoginOutcome, Plane,
};

/// Outbound port used by extension-plane services. Implemented by
/// [`HttpAuthClient`] (production) and by [`crate::AuthService`] (in-process tests).
#[async_trait]
pub trait ProofClient: Send + Sync {
    async fn login(&self, command: LoginCommand) -> Result<LoginOutcome, AuthError>;
    async fn introspect(
        &self,
        token: &str,
        expected: Plane,
    ) -> Result<IntrospectOutcome, AuthError>;
    async fn logout(&self, token: &str, expected: Plane) -> Result<AuthCode, AuthError>;
    async fn enroll_password(
        &self,
        plane: Plane,
        subject_id: Uuid,
        password: &str,
    ) -> Result<(), AuthError>;
    /// Whether the subject has ANY password credential on this plane.
    async fn has_password(&self, plane: Plane, subject_id: Uuid) -> Result<bool, AuthError>;
    async fn revoke_subject(&self, plane: Plane, subject_id: Uuid) -> Result<(), AuthError>;
    async fn complete_challenge(
        &self,
        challenge_id: Uuid,
        code: &str,
    ) -> Result<LoginOutcome, AuthError>;
    /// 短信验证码登录：调用方已验证验证码，凭主体断言请求会话签发。
    async fn login_with_sms_proof(
        &self,
        plane: Plane,
        subject_id: Uuid,
        org_id: Option<Uuid>,
        computer_id: Option<String>,
    ) -> Result<LoginOutcome, AuthError>;
}

/// Reads / mutates proof state on a running `aster-auth` process.
#[derive(Clone)]
pub struct HttpAuthClient {
    base: String,
    internal_token: String,
    http: reqwest::Client,
}

impl HttpAuthClient {
    pub fn new(
        base_url: impl Into<String>,
        internal_token: impl Into<String>,
    ) -> Result<Self, AuthError> {
        let base = base_url.into().trim_end_matches('/').to_string();
        if base.is_empty() {
            return Err(AuthError::storage("auth service URL is empty"));
        }
        Ok(Self {
            base,
            internal_token: internal_token.into(),
            http: reqwest::Client::new(),
        })
    }

    pub fn base_url(&self) -> &str {
        &self.base
    }

    async fn post_envelope(
        &self,
        path: &str,
        body: Value,
        bearer: Option<&str>,
    ) -> Result<(u16, Envelope), AuthError> {
        let url = format!("{}{}", self.base, path);
        let mut req = self.http.post(&url).json(&body);
        if let Some(token) = bearer {
            req = req.header("authorization", format!("Bearer {token}"));
        }
        let response = req
            .send()
            .await
            .map_err(|err| AuthError::storage(format!("auth client: {err}")))?;
        let status = response.status().as_u16();
        let text = response
            .text()
            .await
            .map_err(|err| AuthError::storage(format!("auth client body: {err}")))?;
        let envelope: Envelope = serde_json::from_str(&text).map_err(|err| {
            AuthError::storage(format!(
                "auth client decode failed (HTTP {status}): {err}; body={text}"
            ))
        })?;
        Ok((status, envelope))
    }

    /// Decode a recorded `/v1/login` response body (Wave F3 consumer contract).
    /// Does not touch auth SQL — fixtures only.
    pub fn decode_login_response(status: u16, body: &[u8]) -> Result<LoginOutcome, AuthError> {
        let envelope: Envelope =
            serde_json::from_slice(body).map_err(|err| AuthError::storage(err.to_string()))?;
        Self::parse_login_outcome(status, envelope)
    }

    fn deny_code(message: &str) -> AuthCode {
        AuthCode::parse(message).unwrap_or(AuthCode::InvalidCredentials)
    }

    fn parse_session(data: &Value) -> Result<IssuedSession, AuthError> {
        let subject_id = data
            .get("subject_id")
            .and_then(|v| v.as_str())
            .and_then(|s| Uuid::parse_str(s).ok())
            .ok_or_else(|| AuthError::storage("auth session missing subject_id"))?;
        let plane = data
            .get("plane")
            .and_then(|v| v.as_str())
            .and_then(Plane::parse)
            .ok_or_else(|| AuthError::storage("auth session missing plane"))?;
        let org_id = data
            .get("org_id")
            .and_then(|v| v.as_str())
            .and_then(|s| Uuid::parse_str(s).ok());
        let auth_time = data.get("auth_time").and_then(|v| v.as_i64()).unwrap_or(0);
        let amr = data
            .get("amr")
            .and_then(|v| v.as_array())
            .map(|items| {
                items
                    .iter()
                    .filter_map(|item| item.as_str().map(str::to_owned))
                    .collect()
            })
            .unwrap_or_default();
        let token = data
            .get("token")
            .and_then(|v| v.as_str())
            .ok_or_else(|| AuthError::storage("auth session missing token"))?
            .to_owned();
        let expires_at = data.get("expires_at").and_then(|v| v.as_i64()).unwrap_or(0);
        Ok(IssuedSession {
            token,
            expires_at,
            attestation: Attestation {
                subject_id,
                plane,
                org_id,
                auth_time,
                amr,
            },
        })
    }

    fn parse_login_outcome(status: u16, envelope: Envelope) -> Result<LoginOutcome, AuthError> {
        if envelope.success {
            if envelope
                .data
                .get("challenge_id")
                .and_then(|v| v.as_str())
                .is_some()
            {
                let challenge_id = envelope
                    .data
                    .get("challenge_id")
                    .and_then(|v| v.as_str())
                    .and_then(|s| Uuid::parse_str(s).ok())
                    .ok_or_else(|| AuthError::storage("auth challenge missing id"))?;
                let plane = envelope
                    .data
                    .get("plane")
                    .and_then(|v| v.as_str())
                    .and_then(Plane::parse)
                    .unwrap_or(Plane::User);
                let expires_at = envelope
                    .data
                    .get("expires_at")
                    .and_then(|v| v.as_i64())
                    .unwrap_or(0);
                return Ok(LoginOutcome::Challenge(IssuedChallenge {
                    challenge_id,
                    plane,
                    expires_at,
                }));
            }
            return Ok(LoginOutcome::Session(Self::parse_session(&envelope.data)?));
        }
        if status == 401 && envelope.message == "challenge_required" {
            let challenge_id = envelope
                .data
                .get("challenge_id")
                .and_then(|v| v.as_str())
                .and_then(|s| Uuid::parse_str(s).ok())
                .ok_or_else(|| AuthError::storage("auth challenge missing id"))?;
            let plane = envelope
                .data
                .get("plane")
                .and_then(|v| v.as_str())
                .and_then(Plane::parse)
                .unwrap_or(Plane::User);
            let expires_at = envelope
                .data
                .get("expires_at")
                .and_then(|v| v.as_i64())
                .unwrap_or(0);
            return Ok(LoginOutcome::Challenge(IssuedChallenge {
                challenge_id,
                plane,
                expires_at,
            }));
        }
        Ok(LoginOutcome::Denied(Self::deny_code(&envelope.message)))
    }
}

#[derive(Debug, Deserialize)]
struct Envelope {
    success: bool,
    #[serde(default)]
    message: String,
    #[serde(default)]
    data: Value,
}

#[async_trait]
impl ProofClient for HttpAuthClient {
    async fn login(&self, command: LoginCommand) -> Result<LoginOutcome, AuthError> {
        let (status, envelope) = self
            .post_envelope(
                "/v1/login",
                json!({
                    "plane": command.plane.as_str(),
                    "identifier": command.identifier,
                    "org_id": command.org_id,
                    "password": command.password,
                    "computer_id": command.computer_id,
                }),
                None,
            )
            .await?;
        Self::parse_login_outcome(status, envelope)
    }

    async fn introspect(
        &self,
        token: &str,
        expected: Plane,
    ) -> Result<IntrospectOutcome, AuthError> {
        let (status, envelope) = self
            .post_envelope(
                "/v1/introspect",
                json!({
                    "token": token,
                    "plane": expected.as_str(),
                }),
                None,
            )
            .await?;
        if envelope.success {
            let subject_id = envelope
                .data
                .get("subject_id")
                .and_then(|v| v.as_str())
                .and_then(|s| Uuid::parse_str(s).ok())
                .ok_or_else(|| AuthError::storage("introspect missing subject_id"))?;
            let plane = envelope
                .data
                .get("plane")
                .and_then(|v| v.as_str())
                .and_then(Plane::parse)
                .unwrap_or(expected);
            let org_id = envelope
                .data
                .get("org_id")
                .and_then(|v| v.as_str())
                .and_then(|s| Uuid::parse_str(s).ok());
            let auth_time = envelope
                .data
                .get("auth_time")
                .and_then(|v| v.as_i64())
                .unwrap_or(0);
            let amr = envelope
                .data
                .get("amr")
                .and_then(|v| v.as_array())
                .map(|items| {
                    items
                        .iter()
                        .filter_map(|item| item.as_str().map(str::to_owned))
                        .collect()
                })
                .unwrap_or_default();
            return Ok(IntrospectOutcome::Active(Attestation {
                subject_id,
                plane,
                org_id,
                auth_time,
                amr,
            }));
        }
        let _ = status;
        Ok(IntrospectOutcome::Denied(Self::deny_code(
            &envelope.message,
        )))
    }

    async fn logout(&self, token: &str, expected: Plane) -> Result<AuthCode, AuthError> {
        let (_status, envelope) = self
            .post_envelope(
                "/v1/logout",
                json!({
                    "token": token,
                    "plane": expected.as_str(),
                }),
                None,
            )
            .await?;
        if envelope.success {
            let status = envelope
                .data
                .get("status")
                .and_then(|v| v.as_str())
                .unwrap_or("ok");
            return Ok(AuthCode::parse(status).unwrap_or(AuthCode::Revoked));
        }
        Err(AuthError {
            code: Self::deny_code(&envelope.message),
            message: envelope.message,
        })
    }

    async fn enroll_password(
        &self,
        plane: Plane,
        subject_id: Uuid,
        password: &str,
    ) -> Result<(), AuthError> {
        let (_status, envelope) = self
            .post_envelope(
                "/v1/credentials",
                json!({
                    "plane": plane.as_str(),
                    "subject_id": subject_id,
                    "password": password,
                }),
                Some(self.internal_token.as_str()),
            )
            .await?;
        if envelope.success {
            return Ok(());
        }
        Err(AuthError {
            code: Self::deny_code(&envelope.message),
            message: envelope.message,
        })
    }

    async fn has_password(&self, plane: Plane, subject_id: Uuid) -> Result<bool, AuthError> {
        let url = format!(
            "{}/v1/credentials/{}/subjects/{}/exists",
            self.base,
            plane.as_str(),
            subject_id
        );
        let response = self
            .http
            .get(url)
            .bearer_auth(&self.internal_token)
            .send()
            .await
            .map_err(|err| AuthError {
                code: AuthCode::Storage,
                message: err.to_string(),
            })?;
        if !response.status().is_success() {
            return Err(AuthError {
                code: AuthCode::Storage,
                message: format!("credential existence returned {}", response.status()),
            });
        }
        let body: serde_json::Value = response.json().await.map_err(|err| AuthError {
            code: AuthCode::Storage,
            message: err.to_string(),
        })?;
        Ok(body
            .get("data")
            .and_then(|data| data.get("exists"))
            .and_then(|exists| exists.as_bool())
            .unwrap_or(false))
    }

    async fn revoke_subject(&self, plane: Plane, subject_id: Uuid) -> Result<(), AuthError> {
        let (_status, envelope) = self
            .post_envelope(
                "/v1/revocations",
                json!({
                    "plane": plane.as_str(),
                    "subject_id": subject_id,
                }),
                Some(self.internal_token.as_str()),
            )
            .await?;
        if envelope.success {
            return Ok(());
        }
        Err(AuthError {
            code: Self::deny_code(&envelope.message),
            message: envelope.message,
        })
    }

    async fn complete_challenge(
        &self,
        challenge_id: Uuid,
        code: &str,
    ) -> Result<LoginOutcome, AuthError> {
        let (status, envelope) = self
            .post_envelope(
                &format!("/v1/challenges/{challenge_id}"),
                json!({ "code": code }),
                None,
            )
            .await?;
        Self::parse_login_outcome(status, envelope)
    }

    async fn login_with_sms_proof(
        &self,
        plane: Plane,
        subject_id: Uuid,
        org_id: Option<Uuid>,
        computer_id: Option<String>,
    ) -> Result<LoginOutcome, AuthError> {
        let (status, envelope) = self
            .post_envelope(
                "/v1/sms-sessions",
                json!({
                    "plane": plane.as_str(),
                    "subject_id": subject_id,
                    "org_id": org_id,
                    "computer_id": computer_id,
                }),
                Some(self.internal_token.as_str()),
            )
            .await?;
        Self::parse_login_outcome(status, envelope)
    }
}

#[cfg(test)]
mod contract_tests {
    use super::*;
    use crate::contexts::proof::domain::Plane;

    /// Wave F3: consumer contract against recorded `/v1/login` envelope (no auth SQL).
    #[test]
    fn login_session_envelope_fixture_decodes() {
        let body = include_str!("../fixtures/login_session_success.json");
        let outcome = HttpAuthClient::decode_login_response(200, body.as_bytes())
            .expect("fixture must decode");
        match outcome {
            LoginOutcome::Session(session) => {
                assert_eq!(session.attestation.plane, Plane::User);
                assert_eq!(session.token, "aster_us_fixture_token");
                assert_eq!(session.expires_at, 1_700_003_600);
            }
            other => panic!("expected Session, got {other:?}"),
        }
    }
}
