//! Inbound HTTP adapter. Product JSON uses the aster-api envelope.
//! OIDC discovery, JWKS, authorize redirects, and the token endpoint stay on
//! the OpenID Connect shapes.

use std::sync::Arc;

use aster_boot::{BootError, BootRequest, BootResponse, ResponsePassthrough, Result};
use base64::engine::general_purpose::STANDARD;
use base64::Engine;
use chrono::{SecondsFormat, Utc};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use uuid::Uuid;

use crate::contexts::proof::application::oidc::{AuthorizeRequest, TokenRequest};
use crate::contexts::proof::application::service::AuthService;
use crate::contexts::proof::domain::{AuthCode, IssuedSession, LoginCommand, LoginOutcome, Plane};

use super::InternalGuard;

#[derive(Clone)]
pub struct ProofController {
    pub auth: Arc<AuthService>,
    pub internal: InternalGuard,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct LoginBody {
    plane: String,
    identifier: String,
    #[serde(default)]
    org_id: Option<Uuid>,
    password: String,
    #[serde(default)]
    computer_id: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct IntrospectBody {
    token: String,
    plane: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct EnrollBody {
    plane: String,
    subject_id: Uuid,
    password: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ChallengeBody {
    code: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RevokeBody {
    plane: String,
    subject_id: Uuid,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct SmsSessionBody {
    plane: String,
    subject_id: Uuid,
    org_id: Option<Uuid>,
    #[serde(default)]
    computer_id: Option<String>,
}

#[derive(Debug, Deserialize)]
struct TokenForm {
    grant_type: String,
    #[serde(default)]
    code: Option<String>,
    #[serde(default)]
    redirect_uri: Option<String>,
    #[serde(default)]
    code_verifier: Option<String>,
    #[serde(default)]
    refresh_token: Option<String>,
    #[serde(default)]
    client_id: Option<String>,
    #[serde(default)]
    client_secret: Option<String>,
}

#[derive(Debug, Serialize)]
struct ApiEnvelope {
    success: bool,
    code: i32,
    message: String,
    data: Value,
    request_id: Uuid,
    timestamp: String,
}

#[derive(Debug, Serialize)]
struct HealthBody {
    status: &'static str,
}

#[aster_boot::controller("/")]
impl ProofController {
    #[aster_boot::get("/health")]
    async fn health(&self) -> Result<HealthBody> {
        self.auth
            .ready()
            .await
            .map_err(|err| BootError::ServiceUnavailable(err.to_string()))?;
        if !self.auth.persists() {
            return Err(BootError::ServiceUnavailable(
                "auth store is ephemeral; set ASTER_AUTH_DATABASE_URL".into(),
            ));
        }
        Ok(HealthBody { status: "ok" })
    }

    #[aster_boot::get("/live")]
    async fn live(&self) -> Result<HealthBody> {
        Ok(HealthBody { status: "up" })
    }

    #[aster_boot::post("/v1/login")]
    async fn login(
        &self,
        #[aster_boot::res] response: ResponsePassthrough,
        #[aster_boot::header("x-request-id")] request_id: Option<String>,
        #[aster_boot::body] body: LoginBody,
    ) -> Result<ApiEnvelope> {
        let Some(plane) = Plane::parse(&body.plane) else {
            return fail(&response, &request_id, AuthCode::InvalidRequest);
        };
        match self
            .auth
            .login(LoginCommand {
                plane,
                identifier: body.identifier,
                org_id: body.org_id,
                password: body.password,
                computer_id: body.computer_id,
            })
            .await
        {
            Ok(LoginOutcome::Session(session)) => {
                ok(&response, &request_id, 200, session_data(session))
            }
            Ok(LoginOutcome::Challenge(challenge)) => respond(
                &response,
                &request_id,
                401,
                false,
                40100,
                "challenge_required",
                json!({
                    "challenge_id": challenge.challenge_id,
                    "plane": challenge.plane.as_str(),
                    "expires_at": challenge.expires_at,
                }),
            ),
            Ok(LoginOutcome::Denied(code)) => fail(&response, &request_id, code),
            Err(error) => fail(&response, &request_id, error.code),
        }
    }

    #[aster_boot::post("/v1/introspect")]
    async fn introspect(
        &self,
        #[aster_boot::res] response: ResponsePassthrough,
        #[aster_boot::header("x-request-id")] request_id: Option<String>,
        #[aster_boot::body] body: IntrospectBody,
    ) -> Result<ApiEnvelope> {
        let Some(plane) = Plane::parse(&body.plane) else {
            return fail(&response, &request_id, AuthCode::InvalidRequest);
        };
        match self.auth.introspect(&body.token, plane).await {
            Ok(crate::contexts::proof::domain::IntrospectOutcome::Active(attestation)) => ok(
                &response,
                &request_id,
                200,
                json!({
                    "subject_id": attestation.subject_id,
                    "plane": attestation.plane.as_str(),
                    "org_id": attestation.org_id,
                    "auth_time": attestation.auth_time,
                    "amr": attestation.amr,
                }),
            ),
            Ok(crate::contexts::proof::domain::IntrospectOutcome::Denied(code)) => {
                fail(&response, &request_id, code)
            }
            Err(error) => fail(&response, &request_id, error.code),
        }
    }

    #[aster_boot::post("/v1/logout")]
    async fn logout(
        &self,
        #[aster_boot::res] response: ResponsePassthrough,
        #[aster_boot::header("x-request-id")] request_id: Option<String>,
        #[aster_boot::body] body: IntrospectBody,
    ) -> Result<ApiEnvelope> {
        let Some(plane) = Plane::parse(&body.plane) else {
            return fail(&response, &request_id, AuthCode::InvalidRequest);
        };
        match self.auth.logout(&body.token, plane).await {
            Ok(code) => ok(
                &response,
                &request_id,
                200,
                json!({"status": code.as_str()}),
            ),
            Err(error) => fail(&response, &request_id, error.code),
        }
    }

    /// Credential existence query (internal token;用于免密账号的"设置初始密码"判定).
    #[aster_boot::get("/v1/credentials/{plane}/subjects/{subject_id}/exists")]
    async fn credential_exists(
        &self,
        #[aster_boot::res] response: ResponsePassthrough,
        #[aster_boot::header("authorization")] authorization: Option<String>,
        #[aster_boot::header("x-request-id")] request_id: Option<String>,
        #[aster_boot::param("plane")] plane: String,
        #[aster_boot::param("subject_id")] subject_id: String,
    ) -> Result<ApiEnvelope> {
        if !self.internal.authorize(authorization.as_deref()) {
            return fail(&response, &request_id, AuthCode::Unauthorized);
        }
        let Some(plane) = Plane::parse(&plane) else {
            return fail(&response, &request_id, AuthCode::InvalidRequest);
        };
        let Ok(subject_id) = Uuid::parse_str(&subject_id) else {
            return fail(&response, &request_id, AuthCode::InvalidRequest);
        };
        match self.auth.has_password(plane, subject_id).await {
            Ok(exists) => ok(&response, &request_id, 200, json!({ "exists": exists })),
            Err(error) => fail(&response, &request_id, error.code),
        }
    }

    #[aster_boot::post("/v1/credentials")]
    async fn enroll(
        &self,
        #[aster_boot::res] response: ResponsePassthrough,
        #[aster_boot::header("authorization")] authorization: Option<String>,
        #[aster_boot::header("x-request-id")] request_id: Option<String>,
        #[aster_boot::body] body: EnrollBody,
    ) -> Result<ApiEnvelope> {
        if !self.internal.authorize(authorization.as_deref()) {
            return fail(&response, &request_id, AuthCode::Unauthorized);
        }
        let Some(plane) = Plane::parse(&body.plane) else {
            return fail(&response, &request_id, AuthCode::InvalidRequest);
        };
        match self
            .auth
            .enroll_password(plane, body.subject_id, &body.password)
            .await
        {
            Ok(()) => ok(&response, &request_id, 201, json!({"status": "enrolled"})),
            Err(error) => fail(&response, &request_id, error.code),
        }
    }

    #[aster_boot::post("/v1/challenges/{id}")]
    async fn challenge(
        &self,
        #[aster_boot::res] response: ResponsePassthrough,
        #[aster_boot::header("x-request-id")] request_id: Option<String>,
        #[aster_boot::param("id")] id: String,
        #[aster_boot::body] body: ChallengeBody,
    ) -> Result<ApiEnvelope> {
        let Ok(id) = Uuid::parse_str(&id) else {
            return fail(&response, &request_id, AuthCode::InvalidRequest);
        };
        match self.auth.complete_challenge(id, &body.code).await {
            Ok(LoginOutcome::Session(session)) => {
                ok(&response, &request_id, 200, session_data(session))
            }
            Ok(LoginOutcome::Denied(code)) => fail(&response, &request_id, code),
            Ok(LoginOutcome::Challenge(_)) => {
                fail(&response, &request_id, AuthCode::InvalidChallenge)
            }
            Err(error) => fail(&response, &request_id, error.code),
        }
    }

    /// 短信验证码登录会话签发（调用方已验证验证码；内部凭据保护）。
    #[aster_boot::post("/v1/sms-sessions")]
    async fn sms_session(
        &self,
        #[aster_boot::res] response: ResponsePassthrough,
        #[aster_boot::header("authorization")] authorization: Option<String>,
        #[aster_boot::header("x-request-id")] request_id: Option<String>,
        #[aster_boot::body] body: SmsSessionBody,
    ) -> Result<ApiEnvelope> {
        if !self.internal.authorize(authorization.as_deref()) {
            return fail(&response, &request_id, AuthCode::Unauthorized);
        }
        let Some(plane) = Plane::parse(&body.plane) else {
            return fail(&response, &request_id, AuthCode::InvalidRequest);
        };
        match self
            .auth
            .login_with_sms_proof(plane, body.subject_id, body.org_id, body.computer_id)
            .await
        {
            Ok(LoginOutcome::Session(session)) => {
                ok(&response, &request_id, 200, session_data(session))
            }
            Ok(LoginOutcome::Denied(code)) => fail(&response, &request_id, code),
            Ok(LoginOutcome::Challenge(_)) => {
                fail(&response, &request_id, AuthCode::InvalidChallenge)
            }
            Err(error) => {
                eprintln!("[sms-session] login error debug: {error:?}");
                fail(&response, &request_id, error.code)
            }
        }
    }

    #[aster_boot::post("/v1/revocations")]
    async fn revoke(
        &self,
        #[aster_boot::res] response: ResponsePassthrough,
        #[aster_boot::header("authorization")] authorization: Option<String>,
        #[aster_boot::header("x-request-id")] request_id: Option<String>,
        #[aster_boot::body] body: RevokeBody,
    ) -> Result<ApiEnvelope> {
        if !self.internal.authorize(authorization.as_deref()) {
            return fail(&response, &request_id, AuthCode::Unauthorized);
        }
        let Some(plane) = Plane::parse(&body.plane) else {
            return fail(&response, &request_id, AuthCode::InvalidRequest);
        };
        match self.auth.revoke_subject(plane, body.subject_id).await {
            Ok(()) => ok(&response, &request_id, 200, json!({"status": "revoked"})),
            Err(error) => fail(&response, &request_id, error.code),
        }
    }

    #[aster_boot::get("/.well-known/openid-configuration", raw)]
    async fn discovery(&self) -> Result<BootResponse> {
        BootResponse::json(&self.auth.discovery())
    }

    #[aster_boot::get("/oauth/jwks", raw)]
    async fn jwks(&self) -> Result<BootResponse> {
        BootResponse::json(&self.auth.jwks())
    }

    #[aster_boot::get("/oauth/authorize", raw)]
    async fn authorize_get(
        &self,
        #[aster_boot::header("authorization")] authorization: Option<String>,
        #[aster_boot::header("cookie")] cookie: Option<String>,
        #[aster_boot::query("response_type")] response_type: Option<String>,
        #[aster_boot::query("client_id")] client_id: Option<String>,
        #[aster_boot::query("redirect_uri")] redirect_uri: Option<String>,
        #[aster_boot::query("code_challenge")] code_challenge: Option<String>,
        #[aster_boot::query("code_challenge_method")] code_challenge_method: Option<String>,
        #[aster_boot::query("nonce")] nonce: Option<String>,
        #[aster_boot::query("state")] state: Option<String>,
    ) -> Result<BootResponse> {
        self.authorize(
            authorization,
            cookie,
            response_type,
            client_id,
            redirect_uri,
            code_challenge,
            code_challenge_method,
            nonce,
            state,
        )
        .await
    }

    #[aster_boot::post("/oauth/authorize", raw)]
    async fn authorize_post(
        &self,
        #[aster_boot::header("authorization")] authorization: Option<String>,
        #[aster_boot::header("cookie")] cookie: Option<String>,
        #[aster_boot::query("response_type")] response_type: Option<String>,
        #[aster_boot::query("client_id")] client_id: Option<String>,
        #[aster_boot::query("redirect_uri")] redirect_uri: Option<String>,
        #[aster_boot::query("code_challenge")] code_challenge: Option<String>,
        #[aster_boot::query("code_challenge_method")] code_challenge_method: Option<String>,
        #[aster_boot::query("nonce")] nonce: Option<String>,
        #[aster_boot::query("state")] state: Option<String>,
    ) -> Result<BootResponse> {
        self.authorize(
            authorization,
            cookie,
            response_type,
            client_id,
            redirect_uri,
            code_challenge,
            code_challenge_method,
            nonce,
            state,
        )
        .await
    }

    #[aster_boot::post("/oauth/token", raw)]
    async fn token(&self, #[aster_boot::request] request: BootRequest) -> Result<BootResponse> {
        let form: TokenForm = serde_urlencoded::from_bytes(request.body())
            .map_err(|_| BootError::BadRequest("token request must be form encoded".into()))?;
        let (client_id, client_secret) = match (form.client_id, form.client_secret) {
            (Some(id), Some(secret)) => (id, secret),
            _ => match basic_client(request.header("authorization")) {
                Some(pair) => pair,
                None => return oauth_error(AuthCode::InvalidClient),
            },
        };
        match self
            .auth
            .exchange(TokenRequest {
                grant_type: form.grant_type,
                code: form.code,
                redirect_uri: form.redirect_uri,
                code_verifier: form.code_verifier,
                refresh_token: form.refresh_token,
                client_id,
                client_secret,
            })
            .await
        {
            Ok(tokens) => BootResponse::json(&json!({
                "access_token": tokens.access_token,
                "token_type": tokens.token_type,
                "expires_in": tokens.expires_in,
                "refresh_token": tokens.refresh_token,
                "id_token": tokens.id_token,
                "scope": tokens.scope,
            })),
            Err(error) => oauth_error(error.code),
        }
    }

    #[aster_boot::get("/auth/verify")]
    async fn verify_get(
        &self,
        #[aster_boot::res] response: ResponsePassthrough,
        #[aster_boot::header("authorization")] authorization: Option<String>,
        #[aster_boot::header("cookie")] cookie: Option<String>,
        #[aster_boot::header("x-request-id")] request_id: Option<String>,
        #[aster_boot::header("x-aster-subject")] subject: Option<String>,
        #[aster_boot::header("x-aster-plane")] plane: Option<String>,
        #[aster_boot::header("x-aster-org")] org: Option<String>,
    ) -> Result<ApiEnvelope> {
        let credentials = PresentedCredentials {
            authorization,
            cookie,
        };
        self.verify(response, credentials, request_id, subject, plane, org)
            .await
    }

    #[aster_boot::post("/auth/verify")]
    async fn verify_post(
        &self,
        #[aster_boot::res] response: ResponsePassthrough,
        #[aster_boot::header("authorization")] authorization: Option<String>,
        #[aster_boot::header("cookie")] cookie: Option<String>,
        #[aster_boot::header("x-request-id")] request_id: Option<String>,
        #[aster_boot::header("x-aster-subject")] subject: Option<String>,
        #[aster_boot::header("x-aster-plane")] plane: Option<String>,
        #[aster_boot::header("x-aster-org")] org: Option<String>,
    ) -> Result<ApiEnvelope> {
        let credentials = PresentedCredentials {
            authorization,
            cookie,
        };
        self.verify(response, credentials, request_id, subject, plane, org)
            .await
    }
}

/// Credentials a forward-auth caller may present. Bearer wins over the session cookie.
struct PresentedCredentials {
    authorization: Option<String>,
    cookie: Option<String>,
}

impl PresentedCredentials {
    fn token(&self) -> Option<String> {
        bearer(self.authorization.as_deref()).or_else(|| cookie_token(self.cookie.as_deref()))
    }
}

impl ProofController {
    async fn authorize(
        &self,
        authorization: Option<String>,
        cookie: Option<String>,
        response_type: Option<String>,
        client_id: Option<String>,
        redirect_uri: Option<String>,
        code_challenge: Option<String>,
        code_challenge_method: Option<String>,
        nonce: Option<String>,
        state: Option<String>,
    ) -> Result<BootResponse> {
        let response_type = response_type.unwrap_or_default();
        let state = state.unwrap_or_default();
        if response_type != "code" || state.is_empty() {
            return oauth_error(AuthCode::InvalidRequest);
        }
        let Some(session_token) =
            bearer(authorization.as_deref()).or_else(|| cookie_token(cookie.as_deref()))
        else {
            return oauth_error(AuthCode::Revoked);
        };
        let redirect_uri = redirect_uri.unwrap_or_default();
        match self
            .auth
            .authorize(AuthorizeRequest {
                session_token,
                client_id: client_id.unwrap_or_default(),
                redirect_uri: redirect_uri.clone(),
                code_challenge: code_challenge.unwrap_or_default(),
                code_challenge_method: code_challenge_method.unwrap_or_default(),
                nonce: nonce.unwrap_or_default(),
            })
            .await
        {
            Ok(code) => match redirect_with_code(&redirect_uri, &code, &state) {
                Ok(location) => Ok(BootResponse::redirect(location)),
                Err(error) => oauth_error(error.code),
            },
            Err(error) => oauth_error(error.code),
        }
    }

    async fn verify(
        &self,
        response: ResponsePassthrough,
        credentials: PresentedCredentials,
        request_id: Option<String>,
        subject: Option<String>,
        plane: Option<String>,
        org: Option<String>,
    ) -> Result<ApiEnvelope> {
        let token = credentials.token();
        let decision = match self
            .auth
            .forward_auth(
                token.as_deref(),
                subject.as_deref(),
                plane.as_deref(),
                org.as_deref(),
            )
            .await
        {
            Ok(decision) => decision,
            Err(error) => return fail(&response, &request_id, error.code),
        };
        if !decision.allowed {
            return fail(
                &response,
                &request_id,
                decision.error.unwrap_or(AuthCode::Revoked),
            );
        }
        if let Some(subject) = decision.subject_id {
            response.set_header("x-aster-subject", subject.to_string())?;
        }
        if let Some(plane) = decision.plane {
            response.set_header("x-aster-plane", plane.as_str())?;
        }
        if let Some(org) = decision.org_id {
            response.set_header("x-aster-org", org.to_string())?;
        }
        let credential = "session";
        response.set_header("x-aster-credential", credential)?;
        ok(
            &response,
            &request_id,
            200,
            json!({
                "subject_id": decision.subject_id,
                "plane": decision.plane.map(|plane| plane.as_str().to_owned()),
                "org_id": decision.org_id,
                "credential": credential,
            }),
        )
    }
}

fn ok(
    response: &ResponsePassthrough,
    request_id: &Option<String>,
    status: u16,
    data: Value,
) -> Result<ApiEnvelope> {
    respond(response, request_id, status, true, 0, "OK", data)
}

fn fail(
    response: &ResponsePassthrough,
    request_id: &Option<String>,
    code: AuthCode,
) -> Result<ApiEnvelope> {
    respond(
        response,
        request_id,
        http_status(code),
        false,
        numeric_code(code),
        code.as_str(),
        Value::Null,
    )
}

fn respond(
    response: &ResponsePassthrough,
    request_id: &Option<String>,
    status: u16,
    success: bool,
    code: i32,
    message: &str,
    data: Value,
) -> Result<ApiEnvelope> {
    let request_id = parse_request_id(request_id.as_deref());
    response.set_status(status)?;
    response.set_header("x-request-id", request_id.to_string())?;
    Ok(ApiEnvelope {
        success,
        code,
        message: message.to_owned(),
        data,
        request_id,
        timestamp: Utc::now().to_rfc3339_opts(SecondsFormat::Millis, true),
    })
}

fn parse_request_id(value: Option<&str>) -> Uuid {
    value
        .and_then(|value| Uuid::parse_str(value.trim()).ok())
        .unwrap_or_else(Uuid::new_v4)
}

fn session_data(session: IssuedSession) -> Value {
    json!({
        "subject_id": session.attestation.subject_id,
        "plane": session.attestation.plane.as_str(),
        "org_id": session.attestation.org_id,
        "auth_time": session.attestation.auth_time,
        "amr": session.attestation.amr,
        "token": session.token,
        "expires_at": session.expires_at,
    })
}

fn http_status(code: AuthCode) -> u16 {
    match code {
        AuthCode::InvalidRequest | AuthCode::InvalidPkce => 400,
        AuthCode::NotAdmitted | AuthCode::PlaneMismatch => 403,
        AuthCode::Storage => 500,
        _ => 401,
    }
}

fn numeric_code(code: AuthCode) -> i32 {
    match http_status(code) {
        400 => 40000,
        403 => 40300,
        500 => 50000,
        _ => 40100,
    }
}

fn oauth_error(code: AuthCode) -> Result<BootResponse> {
    BootResponse::json_with_status(401, &json!({"error": code.as_str()}))
}

fn bearer(value: Option<&str>) -> Option<String> {
    let token = value?
        .trim()
        .strip_prefix("Bearer ")
        .or_else(|| value?.trim().strip_prefix("bearer "))?
        .trim();
    if token.is_empty() {
        None
    } else {
        Some(token.to_owned())
    }
}

fn cookie_token(cookie: Option<&str>) -> Option<String> {
    for part in cookie?.split(';') {
        let part = part.trim();
        if let Some(value) = part
            .strip_prefix("aster_us=")
            .or_else(|| part.strip_prefix("aster_ms="))
        {
            if !value.is_empty() {
                return Some(value.to_owned());
            }
        }
    }
    None
}

fn basic_client(value: Option<&str>) -> Option<(String, String)> {
    let encoded = value?.trim().strip_prefix("Basic ")?;
    let decoded = STANDARD.decode(encoded).ok()?;
    let decoded = String::from_utf8(decoded).ok()?;
    let (id, secret) = decoded.split_once(':')?;
    Some((id.to_owned(), secret.to_owned()))
}

fn redirect_with_code(
    redirect_uri: &str,
    code: &str,
    state: &str,
) -> std::result::Result<String, crate::contexts::proof::domain::AuthError> {
    let mut url = url::Url::parse(redirect_uri)
        .map_err(|_| crate::contexts::proof::domain::AuthError::code(AuthCode::InvalidRequest))?;
    url.query_pairs_mut()
        .append_pair("code", code)
        .append_pair("state", state);
    Ok(url.to_string())
}
