//! Proof store port. Infrastructure implements it; the domain does not know SQL.

use async_trait::async_trait;
use uuid::Uuid;

use super::rules::LockoutSnapshot;
use super::{AuthError, Plane};

#[derive(Debug, Clone)]
pub struct CredentialRecord {
    pub plane: Plane,
    pub subject_id: Uuid,
    pub password_hash: String,
    pub totp_secret: Option<Vec<u8>>,
}

#[derive(Debug, Clone)]
pub struct SessionRecord {
    pub token_hash: String,
    pub plane: Plane,
    pub subject_id: Uuid,
    pub org_id: Option<Uuid>,
    pub auth_time: i64,
    pub amr: Vec<String>,
    pub expires_at: i64,
    /// Member sessions name the computer that may keep them. User-plane sessions leave this empty.
    pub computer_id: Option<String>,
}

#[derive(Debug, Clone)]
pub struct ChallengeRecord {
    pub id: Uuid,
    pub plane: Plane,
    pub subject_id: Uuid,
    pub org_id: Option<Uuid>,
    pub auth_time: i64,
    pub expires_at: i64,
    pub computer_id: Option<String>,
}

#[derive(Debug, Clone)]
pub struct AuthEvent {
    pub kind: String,
    pub plane: Plane,
    pub subject_id: Option<Uuid>,
    pub at: i64,
}

#[derive(Debug, Clone)]
pub struct AuthCodeRecord {
    pub code_hash: String,
    pub client_id: String,
    pub redirect_uri: String,
    pub code_challenge: String,
    pub nonce: String,
    pub session: SessionRecord,
    pub expires_at: i64,
}

#[derive(Debug, Clone)]
pub struct RefreshRecord {
    pub token_hash: String,
    pub client_id: String,
    pub session: SessionRecord,
    pub expires_at: i64,
}

#[derive(Debug, Clone)]
pub struct OidcClientRecord {
    pub client_id: String,
    pub secret_hash: String,
    pub redirect_uris: Vec<String>,
}

/// Result of a versioned lockout write. A lost compare leaves the stored row unchanged.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LockoutWrite {
    Applied,
    Lost,
}

#[async_trait]
pub trait AuthStore: Send + Sync {
    async fn put_credential(&self, credential: CredentialRecord) -> Result<(), AuthError>;
    async fn credential(
        &self,
        plane: Plane,
        subject_id: Uuid,
    ) -> Result<Option<CredentialRecord>, AuthError>;
    async fn put_session(&self, session: SessionRecord) -> Result<(), AuthError>;
    async fn session_by_hash(&self, token_hash: &str) -> Result<Option<SessionRecord>, AuthError>;
    async fn delete_session_hash(&self, token_hash: &str) -> Result<(), AuthError>;
    async fn delete_subject_sessions(
        &self,
        plane: Plane,
        subject_id: Uuid,
    ) -> Result<(), AuthError>;
    /// Drops this subject's sessions on every computer except `computer_id`.
    async fn delete_other_computer_sessions(
        &self,
        plane: Plane,
        subject_id: Uuid,
        computer_id: &str,
    ) -> Result<(), AuthError>;
    async fn lockout(&self, key: &str) -> Result<Option<LockoutSnapshot>, AuthError>;
    async fn put_lockout(
        &self,
        key: &str,
        snapshot: LockoutSnapshot,
    ) -> Result<LockoutWrite, AuthError>;
    /// Clock that decides whether `locked_until` is still open. Not the process clock.
    async fn database_unix_secs(&self) -> Result<i64, AuthError>;
    async fn put_challenge(&self, challenge: ChallengeRecord) -> Result<(), AuthError>;
    async fn take_challenge(&self, id: Uuid) -> Result<Option<ChallengeRecord>, AuthError>;
    async fn append_event(&self, event: AuthEvent) -> Result<(), AuthError>;
    async fn events(&self) -> Result<Vec<AuthEvent>, AuthError>;
    async fn put_code(&self, code: AuthCodeRecord) -> Result<(), AuthError>;
    async fn take_code(&self, code_hash: &str) -> Result<Option<AuthCodeRecord>, AuthError>;
    async fn mark_code_used(&self, code_hash: &str) -> Result<(), AuthError>;
    async fn code_was_used(&self, code_hash: &str) -> Result<bool, AuthError>;
    async fn put_refresh(&self, refresh: RefreshRecord) -> Result<(), AuthError>;
    async fn take_refresh(&self, token_hash: &str) -> Result<Option<RefreshRecord>, AuthError>;
    async fn mark_refresh_used(&self, token_hash: &str) -> Result<(), AuthError>;
    async fn refresh_was_used(&self, token_hash: &str) -> Result<bool, AuthError>;
    async fn put_client(&self, client: OidcClientRecord) -> Result<(), AuthError>;
    async fn client(&self, client_id: &str) -> Result<Option<OidcClientRecord>, AuthError>;
}
