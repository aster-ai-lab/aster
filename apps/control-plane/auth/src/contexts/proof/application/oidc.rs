//! Authorization-code + PKCE S256. Refresh tokens are stored only as hashes.

use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine;
use rsa::pkcs1v15::SigningKey;
use rsa::pkcs8::DecodePrivateKey;
use rsa::signature::{SignatureEncoding, Signer};
use serde_json::{Map, Value};
use sha2::{Digest, Sha256};
use subtle::ConstantTimeEq;
use uuid::Uuid;

use super::service::AuthService;
use crate::contexts::proof::domain::repository::{
    AuthCodeRecord, OidcClientRecord, RefreshRecord, SessionRecord,
};
use crate::contexts::proof::domain::rules::{self, expiry, pkce_matches, Expiry};
use crate::contexts::proof::domain::{AuthCode, AuthError};

#[derive(Debug, Clone)]
pub struct OidcClient {
    pub client_id: String,
    pub client_secret: String,
    pub redirect_uris: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct AuthorizeRequest {
    pub session_token: String,
    pub client_id: String,
    pub redirect_uri: String,
    pub code_challenge: String,
    pub code_challenge_method: String,
    pub nonce: String,
}

#[derive(Debug, Clone)]
pub struct TokenRequest {
    pub grant_type: String,
    pub code: Option<String>,
    pub redirect_uri: Option<String>,
    pub code_verifier: Option<String>,
    pub refresh_token: Option<String>,
    pub client_id: String,
    pub client_secret: String,
}

#[derive(Debug, Clone)]
pub struct TokenResponse {
    pub access_token: String,
    pub token_type: String,
    pub expires_in: i64,
    pub refresh_token: String,
    pub id_token: String,
    pub scope: String,
}

impl AuthService {
    pub async fn register_oidc_client(&self, client: OidcClient) -> Result<(), AuthError> {
        self.store
            .put_client(OidcClientRecord {
                client_id: client.client_id,
                secret_hash: secret_hash(&client.client_secret),
                redirect_uris: client.redirect_uris,
            })
            .await
    }

    pub fn jwks(&self) -> Value {
        serde_json::json!({
            "keys": [{
                "kty": "RSA",
                "use": "sig",
                "alg": "RS256",
                "kid": self.signing.kid,
                "n": self.signing.n,
                "e": self.signing.e,
            }]
        })
    }

    pub fn discovery(&self) -> Value {
        let issuer = self.issuer.trim_end_matches('/');
        serde_json::json!({
            "issuer": issuer,
            "authorization_endpoint": format!("{issuer}/oauth/authorize"),
            "token_endpoint": format!("{issuer}/oauth/token"),
            "jwks_uri": format!("{issuer}/oauth/jwks"),
            "response_types_supported": ["code"],
            "subject_types_supported": ["public"],
            "id_token_signing_alg_values_supported": ["RS256"],
            "token_endpoint_auth_methods_supported": ["client_secret_basic", "client_secret_post"],
            "grant_types_supported": ["authorization_code", "refresh_token"],
            "code_challenge_methods_supported": ["S256"],
            "scopes_supported": ["openid", "profile", "email"],
        })
    }

    pub async fn authorize(&self, request: AuthorizeRequest) -> Result<String, AuthError> {
        if request.code_challenge_method != "S256" || request.code_challenge.is_empty() {
            return Err(AuthError::code(AuthCode::InvalidPkce));
        }
        let client = self
            .store
            .client(&request.client_id)
            .await?
            .ok_or_else(|| AuthError::code(AuthCode::InvalidClient))?;
        if !client
            .redirect_uris
            .iter()
            .any(|uri| uri == &request.redirect_uri)
        {
            return Err(AuthError::code(AuthCode::InvalidRequest));
        }
        let plane = rules::plane_of_token(&request.session_token)
            .ok_or_else(|| AuthError::code(AuthCode::PlaneMismatch))?;
        let attestation = match self.introspect(&request.session_token, plane).await? {
            crate::IntrospectOutcome::Active(attestation) => attestation,
            crate::IntrospectOutcome::Denied(code) => return Err(AuthError::code(code)),
        };
        let now = self.clock.unix_secs();
        let code = rules::random_secret();
        self.store
            .put_code(AuthCodeRecord {
                code_hash: rules::hash_token(&code),
                client_id: request.client_id,
                redirect_uri: request.redirect_uri,
                code_challenge: request.code_challenge,
                nonce: request.nonce,
                session: SessionRecord {
                    token_hash: String::new(),
                    plane: attestation.plane,
                    subject_id: attestation.subject_id,
                    org_id: attestation.org_id,
                    auth_time: attestation.auth_time,
                    amr: attestation.amr,
                    expires_at: now.saturating_add(rules::AUTH_CODE_TTL_SECS),
                    computer_id: None,
                },
                expires_at: now.saturating_add(rules::AUTH_CODE_TTL_SECS),
            })
            .await?;
        Ok(code)
    }

    pub async fn exchange(&self, request: TokenRequest) -> Result<TokenResponse, AuthError> {
        let client = self
            .store
            .client(&request.client_id)
            .await?
            .ok_or_else(|| AuthError::code(AuthCode::InvalidClient))?;
        if !secret_matches(&client.secret_hash, &request.client_secret) {
            return Err(AuthError::code(AuthCode::InvalidClient));
        }
        match request.grant_type.as_str() {
            "authorization_code" => self.exchange_code(request).await,
            "refresh_token" => self.exchange_refresh(request).await,
            _ => Err(AuthError::code(AuthCode::InvalidRequest)),
        }
    }

    async fn exchange_code(&self, request: TokenRequest) -> Result<TokenResponse, AuthError> {
        let code = request
            .code
            .filter(|code| !code.is_empty())
            .ok_or_else(|| AuthError::code(AuthCode::InvalidRequest))?;
        let code_hash = rules::hash_token(&code);
        if self.store.code_was_used(&code_hash).await? {
            return Err(AuthError::code(AuthCode::CodeReplay));
        }
        let Some(stored) = self.store.take_code(&code_hash).await? else {
            return Err(AuthError::code(AuthCode::InvalidRequest));
        };
        self.store.mark_code_used(&code_hash).await?;
        let now = self.clock.unix_secs();
        if expiry(stored.expires_at, now) == Expiry::Expired {
            return Err(AuthError::code(AuthCode::Expired));
        }
        if stored.client_id != request.client_id
            || request.redirect_uri.as_deref() != Some(stored.redirect_uri.as_str())
        {
            return Err(AuthError::code(AuthCode::InvalidRequest));
        }
        let verifier = request.code_verifier.unwrap_or_default();
        if !pkce_matches(&stored.code_challenge, &verifier) {
            return Err(AuthError::code(AuthCode::InvalidPkce));
        }
        self.mint_tokens(stored.client_id, stored.session, Some(stored.nonce))
            .await
    }

    async fn exchange_refresh(&self, request: TokenRequest) -> Result<TokenResponse, AuthError> {
        let refresh = request
            .refresh_token
            .filter(|token| !token.is_empty())
            .ok_or_else(|| AuthError::code(AuthCode::InvalidRequest))?;
        let token_hash = rules::hash_token(&refresh);
        if self.store.refresh_was_used(&token_hash).await? {
            return Err(AuthError::code(AuthCode::RefreshReplay));
        }
        let Some(stored) = self.store.take_refresh(&token_hash).await? else {
            return Err(AuthError::code(AuthCode::RefreshReplay));
        };
        self.store.mark_refresh_used(&token_hash).await?;
        if stored.client_id != request.client_id {
            return Err(AuthError::code(AuthCode::InvalidClient));
        }
        if expiry(stored.expires_at, self.clock.unix_secs()) == Expiry::Expired {
            return Err(AuthError::code(AuthCode::Expired));
        }
        self.mint_tokens(stored.client_id, stored.session, None)
            .await
    }

    async fn mint_tokens(
        &self,
        client_id: String,
        session: SessionRecord,
        nonce: Option<String>,
    ) -> Result<TokenResponse, AuthError> {
        let now = self.clock.unix_secs();
        let expires_in = rules::ACCESS_TOKEN_TTL_SECS;
        let mut claims = Map::new();
        claims.insert(
            "iss".into(),
            Value::String(self.issuer.trim_end_matches('/').to_owned()),
        );
        claims.insert("aud".into(), Value::String(client_id.clone()));
        claims.insert("azp".into(), Value::String(client_id.clone()));
        claims.insert("sub".into(), Value::String(session.subject_id.to_string()));
        claims.insert("iat".into(), serde_json::json!(now));
        claims.insert(
            "exp".into(),
            serde_json::json!(now.saturating_add(expires_in)),
        );
        claims.insert("auth_time".into(), serde_json::json!(session.auth_time));
        claims.insert(
            "plane".into(),
            Value::String(session.plane.as_str().to_owned()),
        );
        if let Some(org_id) = session.org_id {
            claims.insert("org_id".into(), Value::String(org_id.to_string()));
        }
        claims.insert(
            "amr".into(),
            Value::Array(session.amr.iter().cloned().map(Value::String).collect()),
        );
        if let Some(nonce) = nonce {
            claims.insert("nonce".into(), Value::String(nonce));
        }
        let claim_names = claims.keys().cloned().collect::<Vec<_>>();
        if !rules::claims_within_allow_list(&claim_names) {
            return Err(AuthError::code(AuthCode::InvalidRequest));
        }
        let id_token = sign_rs256(&self.signing.private_pem, &self.signing.kid, &claims)?;
        let refresh = rules::random_secret();
        self.store
            .put_refresh(RefreshRecord {
                token_hash: rules::hash_token(&refresh),
                client_id: client_id.clone(),
                session: SessionRecord {
                    expires_at: now.saturating_add(rules::REFRESH_TTL_SECS),
                    ..session
                },
                expires_at: now.saturating_add(rules::REFRESH_TTL_SECS),
            })
            .await?;
        let access_token = rules::random_secret();
        Ok(TokenResponse {
            access_token,
            token_type: "Bearer".into(),
            expires_in,
            refresh_token: refresh,
            id_token,
            scope: "openid".into(),
        })
    }
}

pub fn decode_claims(token: &str) -> Result<Map<String, Value>, AuthError> {
    let mut parts = token.split('.');
    let _header = parts
        .next()
        .ok_or_else(|| AuthError::code(AuthCode::InvalidRequest))?;
    let payload = parts
        .next()
        .ok_or_else(|| AuthError::code(AuthCode::InvalidRequest))?;
    let bytes = URL_SAFE_NO_PAD
        .decode(payload)
        .map_err(|_| AuthError::code(AuthCode::InvalidRequest))?;
    let value: Value =
        serde_json::from_slice(&bytes).map_err(|_| AuthError::code(AuthCode::InvalidRequest))?;
    value
        .as_object()
        .cloned()
        .ok_or_else(|| AuthError::code(AuthCode::InvalidRequest))
}

fn sign_rs256(
    private_pem: &str,
    kid: &str,
    claims: &Map<String, Value>,
) -> Result<String, AuthError> {
    let key = rsa::RsaPrivateKey::from_pkcs8_pem(private_pem)
        .map_err(|err| AuthError::storage(err.to_string()))?;
    let signing = SigningKey::<Sha256>::new(key);
    let header = serde_json::json!({"alg": "RS256", "typ": "JWT", "kid": kid});
    let signing_input = format!(
        "{}.{}",
        URL_SAFE_NO_PAD.encode(
            serde_json::to_vec(&header).map_err(|err| AuthError::storage(err.to_string()))?
        ),
        URL_SAFE_NO_PAD
            .encode(serde_json::to_vec(claims).map_err(|err| AuthError::storage(err.to_string()))?)
    );
    let signature = signing.sign(signing_input.as_bytes());
    Ok(format!(
        "{signing_input}.{}",
        URL_SAFE_NO_PAD.encode(signature.to_bytes())
    ))
}

fn secret_hash(secret: &str) -> String {
    hex::encode(Sha256::digest(secret.as_bytes()))
}

fn secret_matches(stored: &str, presented: &str) -> bool {
    let actual = secret_hash(presented);
    if actual.len() != stored.len() {
        return false;
    }
    ConstantTimeEq::ct_eq(actual.as_bytes(), stored.as_bytes()).into()
}

#[allow(dead_code)]
fn _uuid_marker() -> Uuid {
    Uuid::nil()
}
