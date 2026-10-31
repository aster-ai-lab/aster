//! Login, session check, revocation, lockout, and second-factor challenges.

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use argon2::{
    password_hash::{PasswordHash, PasswordHasher, PasswordVerifier, SaltString},
    Argon2,
};
use uuid::Uuid;

use crate::contexts::proof::domain::repository::{
    AuthEvent, AuthStore, ChallengeRecord, CredentialRecord, LockoutWrite, SessionRecord,
};
use crate::contexts::proof::domain::rules::{
    self, expiry, lockout_decision, record_failure, record_success, Expiry, LockoutDecision,
    LockoutSnapshot,
};
use crate::contexts::proof::domain::{
    Admission, Attestation, AuthCode, AuthError, Clock, DirectoryAdmission, ForwardAuthDecision,
    IntrospectOutcome, IssuedChallenge, IssuedSession, LoginCommand, LoginOutcome, Plane,
    SubjectStatus, SystemClock,
};

pub struct AuthService {
    pub(crate) store: Arc<dyn AuthStore>,
    pub(crate) directory: Arc<dyn DirectoryAdmission>,
    pub(crate) clock: Arc<dyn Clock>,
    dummy_hash: String,
    password_checks: AtomicU64,
    pub(crate) issuer: String,
    pub(crate) signing: oidc_key::SigningMaterial,
    persists: bool,
}

pub(crate) mod oidc_key {
    use rsa::pkcs8::{EncodePrivateKey, EncodePublicKey, LineEnding};
    use rsa::traits::PublicKeyParts;
    use rsa::RsaPrivateKey;

    pub struct SigningMaterial {
        pub private_pem: String,
        #[allow(dead_code)]
        pub public_pem: String,
        pub n: String,
        pub e: String,
        pub kid: String,
    }

    impl SigningMaterial {
        pub fn generate() -> Self {
            let mut rng = rand_core::OsRng;
            let key = RsaPrivateKey::new(&mut rng, 2048).expect("RSA key");
            let private_pem = key
                .to_pkcs8_pem(LineEnding::LF)
                .expect("private pem")
                .to_string();
            let public = key.to_public_key();
            let public_pem = public
                .to_public_key_pem(LineEnding::LF)
                .expect("public pem");
            let n = b64(public.n().to_bytes_be());
            let e = b64(public.e().to_bytes_be());
            Self {
                private_pem,
                public_pem,
                n,
                e,
                kid: "aster-auth-1".into(),
            }
        }
    }

    fn b64(bytes: Vec<u8>) -> String {
        use base64::engine::general_purpose::URL_SAFE_NO_PAD;
        use base64::Engine;
        URL_SAFE_NO_PAD.encode(bytes)
    }
}

impl std::fmt::Debug for AuthService {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AuthService")
            .field("issuer", &self.issuer)
            .finish_non_exhaustive()
    }
}

impl AuthService {
    pub fn new(
        store: Arc<dyn AuthStore>,
        directory: Arc<dyn DirectoryAdmission>,
        clock: Arc<dyn Clock>,
        issuer: impl Into<String>,
    ) -> Self {
        let dummy_password = rules::random_hex(24);
        let dummy_hash = hash_password(&dummy_password).expect("dummy password hash");
        Self {
            store,
            directory,
            clock,
            dummy_hash,
            password_checks: AtomicU64::new(0),
            issuer: issuer.into(),
            signing: oidc_key::SigningMaterial::generate(),
            persists: false,
        }
    }

    /// Postgres keeps sessions across restarts. Memory and SQLite do not.
    pub fn with_persistence(mut self, persists: bool) -> Self {
        self.persists = persists;
        self
    }

    pub fn persists(&self) -> bool {
        self.persists
    }

    pub fn system(
        store: Arc<dyn AuthStore>,
        directory: Arc<dyn DirectoryAdmission>,
        issuer: impl Into<String>,
    ) -> Self {
        Self::new(store, directory, Arc::new(SystemClock), issuer)
    }

    pub fn password_verifications(&self) -> u64 {
        self.password_checks.load(Ordering::SeqCst)
    }

    /// Readiness probe: one store round-trip. Used by `GET /health`.
    pub async fn ready(&self) -> Result<(), AuthError> {
        self.store.database_unix_secs().await.map(|_| ())
    }

    /// Resource grants are not a concept this service can write.
    pub fn resource_grant_count(&self) -> u64 {
        0
    }

    pub fn issuer(&self) -> &str {
        &self.issuer
    }

    /// Does the subject hold a password credential on this plane?
    pub async fn has_password(&self, plane: Plane, subject_id: Uuid) -> Result<bool, AuthError> {
        let credential = self.store.credential(plane, subject_id).await?;
        Ok(credential
            .map(|credential| !credential.password_hash.is_empty())
            .unwrap_or(false))
    }

    pub async fn enroll_password(
        &self,
        plane: Plane,
        subject_id: Uuid,
        password: &str,
    ) -> Result<(), AuthError> {
        let password_hash = hash_password(password).map_err(|err| AuthError::storage(err))?;
        let existing = self.store.credential(plane, subject_id).await?;
        self.store
            .put_credential(CredentialRecord {
                plane,
                subject_id,
                password_hash,
                totp_secret: existing.and_then(|credential| credential.totp_secret),
            })
            .await
    }

    pub async fn enroll_totp(&self, plane: Plane, subject_id: Uuid) -> Result<Vec<u8>, AuthError> {
        let mut credential = self
            .store
            .credential(plane, subject_id)
            .await?
            .ok_or_else(|| AuthError::code(AuthCode::InvalidRequest))?;
        let mut secret = vec![0u8; 20];
        rand_core::RngCore::fill_bytes(&mut rand_core::OsRng, &mut secret);
        credential.totp_secret = Some(secret.clone());
        self.store.put_credential(credential).await?;
        Ok(secret)
    }

    pub async fn login(&self, command: LoginCommand) -> Result<LoginOutcome, AuthError> {
        let identifier = normalize_identifier(&command.identifier);
        let lock_key = lock_key(command.plane, command.org_id, &identifier);
        let now = self.clock.unix_secs();
        let lock_now = self.store.database_unix_secs().await?;
        let snapshot = self.store.lockout(&lock_key).await?;
        if let LockoutDecision::Locked { .. } = lockout_decision(snapshot, lock_now) {
            self.event("failure", command.plane, None, now).await?;
            return Ok(LoginOutcome::Denied(AuthCode::Locked));
        }

        let admission = self
            .directory
            .admit(crate::AdmitQuery {
                plane: command.plane,
                identifier: identifier.clone(),
                org_id: command.org_id,
            })
            .await?;
        let credential = match admission.subject_id {
            Some(subject_id) => self.store.credential(command.plane, subject_id).await?,
            None => None,
        };
        let password_ok = match credential.as_ref() {
            Some(credential) => self.verify_password(&credential.password_hash, &command.password),
            None => self.verify_password(&self.dummy_hash, &command.password) && false,
        };
        if !password_ok || credential.is_none() {
            return self
                .note_failure(
                    &lock_key,
                    snapshot,
                    lock_now,
                    command.plane,
                    admission.subject_id,
                )
                .await;
        }
        let subject_id = admission.subject_id.expect("credential implies a subject");
        if !admitted(command.plane, &admission) {
            self.event("failure", command.plane, Some(subject_id), now)
                .await?;
            return Ok(LoginOutcome::Denied(AuthCode::NotAdmitted));
        }
        if !self.clear_lockout(&lock_key, snapshot, lock_now).await? {
            self.event("failure", command.plane, Some(subject_id), now)
                .await?;
            return Ok(LoginOutcome::Denied(AuthCode::Locked));
        }
        let org_id = if command.plane == Plane::User {
            admission.org_id.or(command.org_id)
        } else {
            None
        };
        let computer_id = member_computer(command.plane, command.computer_id.as_deref());
        if command.plane == Plane::Member && computer_id.is_none() {
            return Ok(LoginOutcome::Denied(AuthCode::InvalidRequest));
        }
        if credential
            .as_ref()
            .and_then(|credential| credential.totp_secret.as_ref())
            .is_some()
        {
            let challenge = ChallengeRecord {
                id: Uuid::new_v4(),
                plane: command.plane,
                subject_id,
                org_id,
                auth_time: now,
                expires_at: now.saturating_add(rules::CHALLENGE_TTL_SECS),
                computer_id: computer_id.clone(),
            };
            let issued = IssuedChallenge {
                challenge_id: challenge.id,
                plane: challenge.plane,
                expires_at: challenge.expires_at,
            };
            self.store.put_challenge(challenge).await?;
            return Ok(LoginOutcome::Challenge(issued));
        }
        let session = self
            .issue(
                command.plane,
                subject_id,
                org_id,
                now,
                vec!["pwd".into()],
                computer_id,
            )
            .await?;
        self.event("success", command.plane, Some(subject_id), now)
            .await?;
        Ok(LoginOutcome::Session(session))
    }

    /// 短信验证码登录：调用方（aster-api）已验证验证码，凭主体断言签发会话。
    /// 信任边界与目录准入相同：内部凭据保护的同一 operator 边界。
    pub async fn login_with_sms_proof(
        &self,
        plane: Plane,
        subject_id: Uuid,
        org_id: Option<Uuid>,
        computer_id: Option<String>,
    ) -> Result<LoginOutcome, AuthError> {
        let computer_id = member_computer(plane, computer_id.as_deref());
        if plane == Plane::Member && computer_id.is_none() {
            return Ok(LoginOutcome::Denied(AuthCode::InvalidRequest));
        }
        let now = self.clock.unix_secs();
        let status = self.directory.subject_status(plane, subject_id).await?;
        if !status_allows(plane, &status) {
            return Ok(LoginOutcome::Denied(AuthCode::NotAdmitted));
        }
        let session = self
            .issue(
                plane,
                subject_id,
                org_id,
                now,
                vec!["sms".into()],
                computer_id,
            )
            .await?;
        self.event("success", plane, Some(subject_id), now).await?;
        Ok(LoginOutcome::Session(session))
    }

    pub async fn complete_challenge(
        &self,
        challenge_id: Uuid,
        code: &str,
    ) -> Result<LoginOutcome, AuthError> {
        let now = self.clock.unix_secs();
        let Some(challenge) = self.store.take_challenge(challenge_id).await? else {
            return Ok(LoginOutcome::Denied(AuthCode::InvalidChallenge));
        };
        if expiry(challenge.expires_at, now) == Expiry::Expired {
            return Ok(LoginOutcome::Denied(AuthCode::Expired));
        }
        let credential = self
            .store
            .credential(challenge.plane, challenge.subject_id)
            .await?;
        let Some(secret) = credential.and_then(|credential| credential.totp_secret) else {
            return Ok(LoginOutcome::Denied(AuthCode::InvalidChallenge));
        };
        if !rules::totp_matches(&secret, now, code) {
            self.event("failure", challenge.plane, Some(challenge.subject_id), now)
                .await?;
            return Ok(LoginOutcome::Denied(AuthCode::InvalidChallenge));
        }
        let status = self
            .directory
            .subject_status(challenge.plane, challenge.subject_id)
            .await?;
        if !status_allows(challenge.plane, &status) {
            return Ok(LoginOutcome::Denied(AuthCode::NotAdmitted));
        }
        let computer_id = member_computer(challenge.plane, challenge.computer_id.as_deref());
        if challenge.plane == Plane::Member && computer_id.is_none() {
            return Ok(LoginOutcome::Denied(AuthCode::InvalidRequest));
        }
        let session = self
            .issue(
                challenge.plane,
                challenge.subject_id,
                challenge.org_id,
                challenge.auth_time,
                vec!["pwd".into(), "otp".into()],
                computer_id,
            )
            .await?;
        self.event("success", challenge.plane, Some(challenge.subject_id), now)
            .await?;
        Ok(LoginOutcome::Session(session))
    }

    pub async fn introspect(
        &self,
        token: &str,
        expected: Plane,
    ) -> Result<IntrospectOutcome, AuthError> {
        let Some(presented) = rules::plane_of_token(token) else {
            return Ok(IntrospectOutcome::Denied(AuthCode::PlaneMismatch));
        };
        if presented != expected {
            return Ok(IntrospectOutcome::Denied(AuthCode::PlaneMismatch));
        }
        let token_hash = rules::hash_token(token);
        let Some(session) = self.store.session_by_hash(&token_hash).await? else {
            return Ok(IntrospectOutcome::Denied(AuthCode::Revoked));
        };
        if session.plane != expected {
            return Ok(IntrospectOutcome::Denied(AuthCode::PlaneMismatch));
        }
        if session.plane == Plane::Member && session.computer_id.as_deref().unwrap_or("").is_empty()
        {
            self.store.delete_session_hash(&token_hash).await?;
            return Ok(IntrospectOutcome::Denied(AuthCode::Revoked));
        }
        let now = self.clock.unix_secs();
        if expiry(session.expires_at, now) == Expiry::Expired {
            self.store.delete_session_hash(&token_hash).await?;
            self.event("revocation", session.plane, Some(session.subject_id), now)
                .await?;
            return Ok(IntrospectOutcome::Denied(AuthCode::Expired));
        }
        let status = self
            .directory
            .subject_status(session.plane, session.subject_id)
            .await?;
        if !status_allows(session.plane, &status) {
            self.store
                .delete_subject_sessions(session.plane, session.subject_id)
                .await?;
            self.event("revocation", session.plane, Some(session.subject_id), now)
                .await?;
            return Ok(IntrospectOutcome::Denied(AuthCode::Revoked));
        }
        Ok(IntrospectOutcome::Active(attestation_from(&session)))
    }

    pub async fn logout(&self, token: &str, expected: Plane) -> Result<AuthCode, AuthError> {
        match self.introspect(token, expected).await? {
            IntrospectOutcome::Denied(code) => Ok(code),
            IntrospectOutcome::Active(attestation) => {
                self.store
                    .delete_session_hash(&rules::hash_token(token))
                    .await?;
                self.event(
                    "revocation",
                    attestation.plane,
                    Some(attestation.subject_id),
                    self.clock.unix_secs(),
                )
                .await?;
                Ok(AuthCode::Revoked)
            }
        }
    }

    pub async fn revoke_subject(&self, plane: Plane, subject_id: Uuid) -> Result<(), AuthError> {
        self.store
            .delete_subject_sessions(plane, subject_id)
            .await?;
        self.event(
            "revocation",
            plane,
            Some(subject_id),
            self.clock.unix_secs(),
        )
        .await
    }
}

#[async_trait::async_trait]
impl crate::client::ProofClient for AuthService {
    async fn login(&self, command: LoginCommand) -> Result<LoginOutcome, AuthError> {
        AuthService::login(self, command).await
    }

    async fn introspect(
        &self,
        token: &str,
        expected: Plane,
    ) -> Result<IntrospectOutcome, AuthError> {
        AuthService::introspect(self, token, expected).await
    }

    async fn logout(&self, token: &str, expected: Plane) -> Result<AuthCode, AuthError> {
        AuthService::logout(self, token, expected).await
    }

    async fn enroll_password(
        &self,
        plane: Plane,
        subject_id: Uuid,
        password: &str,
    ) -> Result<(), AuthError> {
        AuthService::enroll_password(self, plane, subject_id, password).await
    }

    async fn revoke_subject(&self, plane: Plane, subject_id: Uuid) -> Result<(), AuthError> {
        AuthService::revoke_subject(self, plane, subject_id).await
    }

    async fn has_password(&self, plane: Plane, subject_id: Uuid) -> Result<bool, AuthError> {
        AuthService::has_password(self, plane, subject_id).await
    }

    async fn complete_challenge(
        &self,
        challenge_id: Uuid,
        code: &str,
    ) -> Result<LoginOutcome, AuthError> {
        AuthService::complete_challenge(self, challenge_id, code).await
    }

    async fn login_with_sms_proof(
        &self,
        plane: Plane,
        subject_id: Uuid,
        org_id: Option<Uuid>,
        computer_id: Option<String>,
    ) -> Result<LoginOutcome, AuthError> {
        AuthService::login_with_sms_proof(self, plane, subject_id, org_id, computer_id).await
    }
}

impl AuthService {
    pub async fn forward_auth(
        &self,
        token: Option<&str>,
        _client_subject: Option<&str>,
        _client_plane: Option<&str>,
        _client_org: Option<&str>,
    ) -> Result<ForwardAuthDecision, AuthError> {
        let Some(token) = token.filter(|token| !token.is_empty()) else {
            return Ok(ForwardAuthDecision {
                allowed: false,
                error: Some(AuthCode::Revoked),
                subject_id: None,
                plane: None,
                org_id: None,
            });
        };
        let Some(plane) = rules::plane_of_token(token) else {
            return Ok(ForwardAuthDecision {
                allowed: false,
                error: Some(AuthCode::PlaneMismatch),
                subject_id: None,
                plane: None,
                org_id: None,
            });
        };
        match self.introspect(token, plane).await? {
            IntrospectOutcome::Active(attestation) => Ok(ForwardAuthDecision {
                allowed: true,
                error: None,
                subject_id: Some(attestation.subject_id),
                plane: Some(attestation.plane),
                org_id: attestation.org_id,
            }),
            IntrospectOutcome::Denied(code) => Ok(ForwardAuthDecision {
                allowed: false,
                error: Some(code),
                subject_id: None,
                plane: None,
                org_id: None,
            }),
        }
    }

    pub async fn events(&self) -> Result<Vec<AuthEvent>, AuthError> {
        self.store.events().await
    }

    async fn issue(
        &self,
        plane: Plane,
        subject_id: Uuid,
        org_id: Option<Uuid>,
        auth_time: i64,
        amr: Vec<String>,
        computer_id: Option<String>,
    ) -> Result<IssuedSession, AuthError> {
        let computer_id = if plane == Plane::Member {
            let Some(computer_id) = computer_id else {
                return Err(AuthError::code(AuthCode::InvalidRequest));
            };
            self.store
                .delete_other_computer_sessions(plane, subject_id, &computer_id)
                .await?;
            Some(computer_id)
        } else {
            None
        };
        let token = rules::random_token(plane);
        let expires_at = self
            .clock
            .unix_secs()
            .saturating_add(rules::SESSION_TTL_SECS);
        let session = SessionRecord {
            token_hash: rules::hash_token(&token),
            plane,
            subject_id,
            org_id,
            auth_time,
            amr: amr.clone(),
            expires_at,
            computer_id,
        };
        let attestation = attestation_from(&session);
        self.store.put_session(session).await?;
        Ok(IssuedSession {
            token,
            expires_at,
            attestation,
        })
    }

    fn verify_password(&self, hash: &str, password: &str) -> bool {
        self.password_checks.fetch_add(1, Ordering::SeqCst);
        let Ok(parsed) = PasswordHash::new(hash) else {
            return false;
        };
        Argon2::default()
            .verify_password(password.as_bytes(), &parsed)
            .is_ok()
    }

    /// A lost compare is re-read. It does not lower a committed failure count.
    async fn note_failure(
        &self,
        lock_key: &str,
        mut snapshot: Option<LockoutSnapshot>,
        now: i64,
        plane: Plane,
        subject_id: Option<Uuid>,
    ) -> Result<LoginOutcome, AuthError> {
        for _ in 0..4 {
            if let LockoutDecision::Locked { .. } = lockout_decision(snapshot, now) {
                self.event("failure", plane, subject_id, now).await?;
                return Ok(LoginOutcome::Denied(AuthCode::Locked));
            }
            let next = record_failure(snapshot, now);
            match self.store.put_lockout(lock_key, next).await? {
                LockoutWrite::Applied => {
                    self.event("failure", plane, subject_id, now).await?;
                    return Ok(LoginOutcome::Denied(AuthCode::InvalidCredentials));
                }
                LockoutWrite::Lost => {
                    snapshot = self.store.lockout(lock_key).await?;
                }
            }
        }
        if let LockoutDecision::Locked { .. } = lockout_decision(snapshot, now) {
            self.event("failure", plane, subject_id, now).await?;
            return Ok(LoginOutcome::Denied(AuthCode::Locked));
        }
        self.event("failure", plane, subject_id, now).await?;
        Ok(LoginOutcome::Denied(AuthCode::InvalidCredentials))
    }

    /// Returns false when the clear loses the compare. A lost clear is not retried:
    /// a second write could zero a newer failure count that is not yet a lock window.
    async fn clear_lockout(
        &self,
        lock_key: &str,
        snapshot: Option<LockoutSnapshot>,
        now: i64,
    ) -> Result<bool, AuthError> {
        if let LockoutDecision::Locked { .. } = lockout_decision(snapshot, now) {
            return Ok(false);
        }
        match self
            .store
            .put_lockout(lock_key, record_success(snapshot))
            .await?
        {
            LockoutWrite::Applied => Ok(true),
            LockoutWrite::Lost => Ok(false),
        }
    }

    pub(crate) async fn event(
        &self,
        kind: &str,
        plane: Plane,
        subject_id: Option<Uuid>,
        at: i64,
    ) -> Result<(), AuthError> {
        self.store
            .append_event(AuthEvent {
                kind: kind.to_owned(),
                plane,
                subject_id,
                at,
            })
            .await
    }
}

fn attestation_from(session: &SessionRecord) -> Attestation {
    Attestation {
        subject_id: session.subject_id,
        plane: session.plane,
        org_id: session.org_id,
        auth_time: session.auth_time,
        amr: session.amr.clone(),
    }
}

fn admitted(plane: Plane, admission: &Admission) -> bool {
    let Some(_) = admission.subject_id else {
        return false;
    };
    if !admission.active {
        return false;
    }
    plane != Plane::User || admission.console_login
}

pub(crate) fn status_allows(plane: Plane, status: &SubjectStatus) -> bool {
    status.active && (plane != Plane::User || status.console_login)
}

fn normalize_identifier(raw: &str) -> String {
    raw.trim().to_lowercase()
}

/// Member logins keep a normalized computer id. Other planes never bind one.
fn member_computer(plane: Plane, raw: Option<&str>) -> Option<String> {
    if plane == Plane::Member {
        crate::contexts::proof::domain::normalize_computer_id(raw)
    } else {
        None
    }
}

fn lock_key(plane: Plane, org_id: Option<Uuid>, identifier: &str) -> String {
    format!(
        "{}:{}:{identifier}",
        plane.as_str(),
        org_id
            .map(|id| id.to_string())
            .unwrap_or_else(|| "-".into())
    )
}

pub(crate) fn hash_password(password: &str) -> Result<String, String> {
    let salt = SaltString::generate(&mut rand_core::OsRng);
    Argon2::default()
        .hash_password(password.as_bytes(), &salt)
        .map(|hash| hash.to_string())
        .map_err(|err| err.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::contexts::proof::domain::rules::claims_within_allow_list;
    use crate::contexts::proof::domain::{AdmitQuery, ManualClock};
    use crate::contexts::proof::infrastructure::memory::MemoryAuthStore;
    use std::sync::Mutex;

    struct Dir {
        inner: Mutex<DirState>,
    }

    #[derive(Clone)]
    struct Person {
        subject_id: Uuid,
        org_id: Option<Uuid>,
        identifier: String,
        active: bool,
        console_login: bool,
    }

    struct DirState {
        people: Vec<Person>,
    }

    impl Dir {
        fn new(people: Vec<Person>) -> Self {
            Self {
                inner: Mutex::new(DirState { people }),
            }
        }

        fn set_active(&self, subject_id: Uuid, active: bool) {
            for person in &mut self.inner.lock().expect("dir").people {
                if person.subject_id == subject_id {
                    person.active = active;
                }
            }
        }
    }

    #[async_trait::async_trait]
    impl DirectoryAdmission for Dir {
        async fn admit(&self, query: AdmitQuery) -> Result<Admission, AuthError> {
            let state = self.inner.lock().expect("dir");
            let person = state.people.iter().find(|person| {
                person.identifier == query.identifier
                    && (query.plane == Plane::Member || person.org_id == query.org_id)
            });
            Ok(match person {
                None => Admission::unknown(),
                Some(person) => Admission {
                    subject_id: Some(person.subject_id),
                    org_id: person.org_id,
                    active: person.active,
                    console_login: person.console_login,
                },
            })
        }

        async fn subject_status(
            &self,
            _plane: Plane,
            subject_id: Uuid,
        ) -> Result<SubjectStatus, AuthError> {
            let state = self.inner.lock().expect("dir");
            let person = state
                .people
                .iter()
                .find(|person| person.subject_id == subject_id);
            Ok(match person {
                None => SubjectStatus {
                    active: false,
                    console_login: false,
                    org_id: None,
                },
                Some(person) => SubjectStatus {
                    active: person.active,
                    console_login: person.console_login,
                    org_id: person.org_id,
                },
            })
        }
    }

    fn service(dir: Arc<Dir>, clock: Arc<ManualClock>) -> AuthService {
        AuthService::new(
            Arc::new(MemoryAuthStore::new()),
            dir,
            clock,
            "https://auth.aster.test",
        )
    }

    #[tokio::test]
    async fn proof_rules_cover_plane_admission_revocation_lockout_and_claims() {
        let org_id = Uuid::new_v4();
        let member_id = Uuid::new_v4();
        let user_id = Uuid::new_v4();
        let blocked_id = Uuid::new_v4();
        let dir = Arc::new(Dir::new(vec![
            Person {
                subject_id: member_id,
                org_id: None,
                identifier: "member@example.com".into(),
                active: true,
                console_login: false,
            },
            Person {
                subject_id: user_id,
                org_id: Some(org_id),
                identifier: "admin@example.com".into(),
                active: true,
                console_login: true,
            },
            Person {
                subject_id: blocked_id,
                org_id: Some(org_id),
                identifier: "staff@example.com".into(),
                active: true,
                console_login: false,
            },
        ]));
        let clock = Arc::new(ManualClock::new(1_700_000_000));
        let auth = service(Arc::clone(&dir), Arc::clone(&clock));
        auth.enroll_password(Plane::Member, member_id, "member-password")
            .await
            .unwrap();
        auth.enroll_password(Plane::User, user_id, "user-password")
            .await
            .unwrap();
        auth.enroll_password(Plane::User, blocked_id, "staff-password")
            .await
            .unwrap();

        let before = auth.password_verifications();
        let unknown = auth
            .login(LoginCommand {
                plane: Plane::Member,
                identifier: "missing@example.com".into(),
                org_id: None,
                password: "whatever-password".into(),

                computer_id: None,
            })
            .await
            .unwrap();
        assert!(matches!(
            unknown,
            LoginOutcome::Denied(AuthCode::InvalidCredentials)
        ));
        assert!(auth.password_verifications() > before);

        let member = auth
            .login(LoginCommand {
                plane: Plane::Member,
                identifier: "member@example.com".into(),
                org_id: None,
                password: "member-password".into(),
                computer_id: Some("computer-proof-1".into()),
            })
            .await
            .unwrap();
        let LoginOutcome::Session(member) = member else {
            panic!("member login");
        };
        assert!(member.token.starts_with("aster_ms_"));
        assert_eq!(member.attestation.subject_id, member_id);
        assert!(member.attestation.org_id.is_none());
        assert!(claims_within_allow_list(&member.attestation.claim_names()));

        let crossed = auth.introspect(&member.token, Plane::User).await.unwrap();
        assert!(matches!(
            crossed,
            IntrospectOutcome::Denied(AuthCode::PlaneMismatch)
        ));

        let user = auth
            .login(LoginCommand {
                plane: Plane::User,
                identifier: "admin@example.com".into(),
                org_id: Some(org_id),
                password: "user-password".into(),

                computer_id: None,
            })
            .await
            .unwrap();
        let LoginOutcome::Session(user) = user else {
            panic!("user login");
        };
        assert!(user.token.starts_with("aster_us_"));
        assert_eq!(user.attestation.org_id, Some(org_id));
        assert!(matches!(
            auth.introspect(&user.token, Plane::Member).await.unwrap(),
            IntrospectOutcome::Denied(AuthCode::PlaneMismatch)
        ));

        let staff = auth
            .login(LoginCommand {
                plane: Plane::User,
                identifier: "staff@example.com".into(),
                org_id: Some(org_id),
                password: "staff-password".into(),

                computer_id: None,
            })
            .await
            .unwrap();
        assert!(matches!(staff, LoginOutcome::Denied(AuthCode::NotAdmitted)));

        auth.logout(&member.token, Plane::Member).await.unwrap();
        assert!(matches!(
            auth.introspect(&member.token, Plane::Member).await.unwrap(),
            IntrospectOutcome::Denied(AuthCode::Revoked)
        ));

        clock.set(user.expires_at);
        assert!(matches!(
            auth.introspect(&user.token, Plane::User).await.unwrap(),
            IntrospectOutcome::Denied(AuthCode::Expired)
        ));

        clock.set(1_700_000_100);
        let again = auth
            .login(LoginCommand {
                plane: Plane::User,
                identifier: "admin@example.com".into(),
                org_id: Some(org_id),
                password: "user-password".into(),

                computer_id: None,
            })
            .await
            .unwrap();
        let LoginOutcome::Session(again) = again else {
            panic!("relogin");
        };
        dir.set_active(user_id, false);
        assert!(matches!(
            auth.introspect(&again.token, Plane::User).await.unwrap(),
            IntrospectOutcome::Denied(AuthCode::Revoked)
        ));
        dir.set_active(user_id, true);
        let resigned = auth
            .login(LoginCommand {
                plane: Plane::User,
                identifier: "admin@example.com".into(),
                org_id: Some(org_id),
                password: "user-password".into(),

                computer_id: None,
            })
            .await
            .unwrap();
        let LoginOutcome::Session(resigned) = resigned else {
            panic!("resign login");
        };
        dir.set_active(user_id, false);
        assert!(matches!(
            auth.introspect(&resigned.token, Plane::User).await.unwrap(),
            IntrospectOutcome::Denied(AuthCode::Revoked)
        ));

        let shared = Arc::new(MemoryAuthStore::new());
        let clock = Arc::new(ManualClock::new(50));
        let first = AuthService::new(
            shared.clone(),
            Arc::clone(&dir) as Arc<dyn DirectoryAdmission>,
            Arc::clone(&clock) as Arc<dyn Clock>,
            "https://auth.aster.test",
        );
        let second = AuthService::new(shared, dir, clock, "https://auth.aster.test");
        for _ in 0..rules::LOCKOUT_FAILURES {
            let denied = first
                .login(LoginCommand {
                    plane: Plane::Member,
                    identifier: "member@example.com".into(),
                    org_id: None,
                    password: "wrong-password".into(),

                    computer_id: None,
                })
                .await
                .unwrap();
            assert!(matches!(
                denied,
                LoginOutcome::Denied(AuthCode::InvalidCredentials)
            ));
        }
        let locked = second
            .login(LoginCommand {
                plane: Plane::Member,
                identifier: "member@example.com".into(),
                org_id: None,
                password: "member-password".into(),

                computer_id: None,
            })
            .await
            .unwrap();
        assert!(matches!(locked, LoginOutcome::Denied(AuthCode::Locked)));

        let secret = auth.enroll_totp(Plane::Member, member_id).await.unwrap();
        let challenged = auth
            .login(LoginCommand {
                plane: Plane::Member,
                identifier: "member@example.com".into(),
                org_id: None,
                password: "member-password".into(),
                computer_id: Some("computer-proof-1".into()),
            })
            .await
            .unwrap();
        let LoginOutcome::Challenge(challenge) = challenged else {
            panic!("expected challenge before a session");
        };
        let code = format!("{:06}", rules::totp_code(&secret, auth.clock.unix_secs()));
        let completed = auth
            .complete_challenge(challenge.challenge_id, &code)
            .await
            .unwrap();
        let LoginOutcome::Session(session) = completed else {
            panic!("challenge should mint one session");
        };
        assert_eq!(
            session.attestation.amr,
            vec!["pwd".to_string(), "otp".to_string()]
        );
        let replay = auth
            .complete_challenge(challenge.challenge_id, &code)
            .await
            .unwrap();
        assert!(matches!(
            replay,
            LoginOutcome::Denied(AuthCode::InvalidChallenge)
        ));
        assert_eq!(auth.resource_grant_count(), 0);
        let events = auth.events().await.unwrap();
        assert!(events.iter().any(|event| event.kind == "success"));
        assert!(events.iter().any(|event| event.kind == "failure"));
        assert!(events.iter().any(|event| event.kind == "revocation"));
    }

    async fn member_password(
        auth: &AuthService,
        identifier: &str,
        password: &str,
        computer: Option<&str>,
    ) -> LoginOutcome {
        auth.login(LoginCommand {
            plane: Plane::Member,
            identifier: identifier.into(),
            org_id: None,
            password: password.into(),
            computer_id: computer.map(str::to_owned),
        })
        .await
        .unwrap()
    }

    fn issued(outcome: LoginOutcome) -> IssuedSession {
        match outcome {
            LoginOutcome::Session(session) => session,
            other => panic!("expected a session, got {other:?}"),
        }
    }

    async fn member_active(auth: &AuthService, token: &str) -> bool {
        matches!(
            auth.introspect(token, Plane::Member).await.unwrap(),
            IntrospectOutcome::Active(_)
        )
    }

    #[tokio::test]
    async fn member_computer_session_rule() {
        let account_a = Uuid::new_v4();
        let account_b = Uuid::new_v4();
        let account_c = Uuid::new_v4();
        let user_id = Uuid::new_v4();
        let org_id = Uuid::new_v4();
        let dir = Arc::new(Dir::new(vec![
            Person {
                subject_id: account_a,
                org_id: None,
                identifier: "a@example.com".into(),
                active: true,
                console_login: false,
            },
            Person {
                subject_id: account_b,
                org_id: None,
                identifier: "b@example.com".into(),
                active: true,
                console_login: false,
            },
            Person {
                subject_id: account_c,
                org_id: None,
                identifier: "c@example.com".into(),
                active: true,
                console_login: false,
            },
            Person {
                subject_id: user_id,
                org_id: Some(org_id),
                identifier: "admin@example.com".into(),
                active: true,
                console_login: true,
            },
        ]));
        let auth = service(dir, Arc::new(ManualClock::new(1_700_000_000)));
        auth.enroll_password(Plane::Member, account_a, "password-a")
            .await
            .unwrap();
        auth.enroll_password(Plane::Member, account_c, "password-c")
            .await
            .unwrap();
        auth.enroll_password(Plane::User, user_id, "password-user")
            .await
            .unwrap();

        let a_on_1 =
            issued(member_password(&auth, "a@example.com", "password-a", Some("computer-1")).await);
        assert!(member_active(&auth, &a_on_1.token).await);

        let user = issued(
            auth.login(LoginCommand {
                plane: Plane::User,
                identifier: "admin@example.com".into(),
                org_id: Some(org_id),
                password: "password-user".into(),
                computer_id: None,
            })
            .await
            .unwrap(),
        );

        let c_on_1 =
            issued(member_password(&auth, "c@example.com", "password-c", Some("computer-1")).await);
        let a_on_2 =
            issued(member_password(&auth, "a@example.com", "password-a", Some("computer-2")).await);
        assert!(!member_active(&auth, &a_on_1.token).await);
        assert!(member_active(&auth, &a_on_2.token).await);
        assert!(member_active(&auth, &c_on_1.token).await);

        let a_on_2_again =
            issued(member_password(&auth, "a@example.com", "password-a", Some("computer-2")).await);
        assert!(member_active(&auth, &a_on_2.token).await);
        assert!(member_active(&auth, &a_on_2_again.token).await);

        let b_on_2 = issued(
            auth.login_with_sms_proof(Plane::Member, account_b, None, Some("computer-2".into()))
                .await
                .unwrap(),
        );
        assert!(member_active(&auth, &b_on_2.token).await);
        assert!(member_active(&auth, &a_on_2.token).await);
        assert!(member_active(&auth, &c_on_1.token).await);
        assert!(matches!(
            auth.introspect(&user.token, Plane::User).await.unwrap(),
            IntrospectOutcome::Active(_)
        ));

        let missing = member_password(&auth, "a@example.com", "password-a", None).await;
        assert!(matches!(
            missing,
            LoginOutcome::Denied(AuthCode::InvalidRequest)
        ));
        let blank = member_password(&auth, "a@example.com", "password-a", Some("  ")).await;
        assert!(matches!(
            blank,
            LoginOutcome::Denied(AuthCode::InvalidRequest)
        ));
        assert!(member_active(&auth, &a_on_2.token).await);

        let sms_missing = auth
            .login_with_sms_proof(Plane::Member, account_a, None, None)
            .await
            .unwrap();
        assert!(matches!(
            sms_missing,
            LoginOutcome::Denied(AuthCode::InvalidRequest)
        ));
        let a_sms_on_1 = issued(
            auth.login_with_sms_proof(Plane::Member, account_a, None, Some("computer-1".into()))
                .await
                .unwrap(),
        );
        assert!(a_sms_on_1.token.starts_with("aster_ms_"));
        assert!(!member_active(&auth, &a_on_2.token).await);
        assert!(!member_active(&auth, &a_on_2_again.token).await);
        assert!(member_active(&auth, &a_sms_on_1.token).await);
        assert!(member_active(&auth, &b_on_2.token).await);
        assert!(member_active(&auth, &c_on_1.token).await);
    }
}
